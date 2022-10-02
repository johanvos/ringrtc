use std::collections::HashMap;

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common::{CallDirection, CallId, CallMediaType, DeviceId, Result};
use crate::core::bandwidth_mode::BandwidthMode;
use crate::core::call_manager::CallManager;
use crate::core::group_call;
use crate::core::group_call::{GroupId, SignalingMessageUrgency};
use crate::core::signaling;
use crate::core::util::{ptr_as_mut};

use crate::java::jtypes::{JString,JArrayByte,JByteArray,JArrayByte2D,JByteArray2D};

use crate::lite::http;
use crate::lite::sfu::{UserId};
use crate::native::{CallState,CallStateHandler,GroupUpdate,GroupUpdateHandler,NativeCallContext,NativePlatform,PeerId,SignalingSender,};
use crate::webrtc::logging;
use crate::webrtc::media::{
    AudioTrack, VideoFrame, VideoSink, VideoSource, VideoTrack,
};

use crate::webrtc::peer_connection::AudioLevel;

use crate::webrtc::peer_connection_factory::{
    self as pcf, IceServer, PeerConnectionFactory
};
use crate::webrtc::peer_connection_observer::NetworkRoute;

fn init_logging() {
    env_logger::builder()
        .filter(None, log::LevelFilter::Debug)
        .init();
    println!("LOGINIT done");
    // let is_first_time_initializing_logger = log::set_logger(&LOG).is_ok();
    let is_first_time_initializing_logger = true;
    println!("EXTRALOG? {}", is_first_time_initializing_logger);
    if is_first_time_initializing_logger {
        // log::set_max_level(log::LevelFilter::Debug);
        logging::set_logger(log::LevelFilter::Warn);
        println!("EXTRALOG? yes");
    }
    // logging::set_logger(log::LevelFilter::Trace);
    info!("INFO logging enabled");
}

// When the Java layer processes events, we want everything to go through a common queue that
// combines all the things we want to "push" to it.
pub enum Event {
    // The JavaScript should send the following signaling message to the given
    // PeerId in context of the given CallId.  If the DeviceId is None, then
    // broadcast to all devices of that PeerId.
    SendSignaling(PeerId, Option<DeviceId>, CallId, signaling::Message),
    // The JavaScript should send the following opaque call message to the
    // given recipient UUID.
    SendCallMessage {
        recipient_uuid: UserId,
        message: Vec<u8>,
        urgency: group_call::SignalingMessageUrgency,
    },
    // The JavaScript should send the following opaque call message to all
    // other members of the given group
    SendCallMessageToGroup {
        group_id: GroupId,
        message: Vec<u8>,
        urgency: group_call::SignalingMessageUrgency,
    },
    // The call with the given remote PeerId has changed state.
    // We assume only one call per remote PeerId at a time.
    CallState(PeerId, CallId, CallState),
    // The state of the remote video (whether enabled or not) changed.
    // Like call state, we ID the call by PeerId and assume there is only one.
    RemoteVideoStateChange(PeerId, bool),
    // Whether the remote is sharing its screen or not changed.
    // Like call state, we ID the call by PeerId and assume there is only one.
    RemoteSharingScreenChange(PeerId, bool),
    // The group call has an update.
    GroupUpdate(GroupUpdate),
    // JavaScript should initiate an HTTP request.
    SendHttpRequest {
        request_id: u32,
        request: http::Request,
    },
    // The network route changed for a 1:1 call
    NetworkRouteChange(PeerId, NetworkRoute),
    AudioLevels {
        peer_id: PeerId,
        captured_level: AudioLevel,
        received_level: AudioLevel,
    },
}

/// Wraps a [`std::sync::mpsc::Sender`] with a callback to report new events.
#[derive(Clone)]
#[repr(C)]
#[allow(non_snake_case)]
struct EventReporter {
    pub statusCallback: unsafe extern "C" fn(CallId, u64, i32, CallMediaType),
    pub answerCallback: unsafe extern "C" fn(JArrayByte),
    pub iceUpdateCallback: unsafe extern "C" fn(JArrayByte2D),
    sender: Sender<Event>,
    report: Arc<dyn Fn() + Send + Sync>,
}

impl EventReporter {
    fn new(statusCallback: extern "C" fn(CallId, u64, i32, CallMediaType),
           answerCallback: extern "C" fn(JArrayByte),
           iceUpdateCallback: extern "C" fn(JArrayByte2D),
            sender: Sender<Event>, report: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            statusCallback,
            answerCallback,
            iceUpdateCallback,
            sender,
            report: Arc::new(report),
        }
    }

    fn send(&self, event: Event) -> Result<()> {
        match event  {
            Event::SendSignaling(_peer_id, _maybe_device_id, _call_id, signal) => {
                info!("[JV] SendSignalingEvent");
                match signal {
                    signaling::Message::Answer(answer) => {
                        info!("[JV] SendSignaling ANSWER Event");
                        let op = JArrayByte::new(answer.opaque);
                        unsafe {
                            (self.answerCallback)(op);
                        }
                    }
                    signaling::Message::Ice(ice) => {
                        info!("[JV] SendSignaling ICE Event");
                        let icepack: JArrayByte2D = JArrayByte2D::new(ice.candidates);
                        unsafe {
                            (self.iceUpdateCallback)(icepack);
                        }
                    }
                    _ => {
                        info!("[JV] unknownSendSignalingEvent WHICH IS WHAT WE NEED TO FIX NOW!");
                    }
                }
            }
            Event::CallState(_peer_id, call_id, CallState::Incoming(call_media_type)) => {
                info!("[JV] CALLSTATEEVEMNT");
                let direction = 0;
                unsafe {
                    (self.statusCallback)(call_id, 1,direction, call_media_type);
                }
            }
            Event::CallState(_peer_id, call_id, state) => {
                info!("[JV] CallState changed");
                let (state_string, state_index) = match state {
                    CallState::Ringing => ("ringing", 1),
                    CallState::Connected => ("connected", 2),
                    CallState::Connecting => ("connecting", 3),
                    CallState::Concluded => ("Concluded", 4),
                    CallState::Incoming(_) => ("incoming", 5),
                    CallState::Outgoing(_) => ("outgoing", 6),
                    CallState::Ended(_) => ("ended", 7),
                };
                info!("New state = {} and index = {}", state_string, state_index);
                unsafe {
                    (self.statusCallback)(call_id, 1, 10*state_index, CallMediaType::Audio);
                }
            }
            _ => {
                info!("[JV] unknownevent");
            }
        };

        Ok(())
    }

    fn report(&self) {
        (self.report)();
    }
}

impl SignalingSender for EventReporter {
    fn send_signaling(
        &self,
        recipient_id: &str,
        call_id: CallId,
        receiver_device_id: Option<DeviceId>,
        msg: signaling::Message,
    ) -> Result<()> {
info!("Need to send SIGNALING msg {:?}", msg);
        self.send(Event::SendSignaling(
            recipient_id.to_string(),
            receiver_device_id,
            call_id,
            msg,
        ))?;
        Ok(())
    }

    fn send_call_message(
        &self,
        recipient_uuid: UserId,
        message: Vec<u8>,
        urgency: SignalingMessageUrgency,
    ) -> Result<()> {
        self.send(Event::SendCallMessage {
            recipient_uuid,
            message,
            urgency,
        })?;
        Ok(())
    }

    fn send_call_message_to_group(
        &self,
        group_id: GroupId,
        message: Vec<u8>,
        urgency: group_call::SignalingMessageUrgency,
    ) -> Result<()> {
        self.send(Event::SendCallMessageToGroup {
            group_id,
            message,
            urgency,
        })?;
        Ok(())
    }
}

impl CallStateHandler for EventReporter {
    fn handle_call_state(
        &self,
        remote_peer_id: &str,
        call_id: CallId,
        call_state: CallState,
    ) -> Result<()> {
info!("[JV] CallStatehandler, invoke self.send");

        self.send(Event::CallState(
            remote_peer_id.to_string(),
            call_id,
            call_state,
        ))?;
        Ok(())
    }

    fn handle_network_route(
        &self,
        remote_peer_id: &str,
        network_route: NetworkRoute,
    ) -> Result<()> {
        self.send(Event::NetworkRouteChange(
            remote_peer_id.to_string(),
            network_route,
        ))?;
        Ok(())
    }

    fn handle_remote_video_state(&self, remote_peer_id: &str, enabled: bool) -> Result<()> {
        self.send(Event::RemoteVideoStateChange(
            remote_peer_id.to_string(),
            enabled,
        ))?;
        Ok(())
    }

    fn handle_remote_sharing_screen(&self, remote_peer_id: &str, enabled: bool) -> Result<()> {
        self.send(Event::RemoteSharingScreenChange(
            remote_peer_id.to_string(),
            enabled,
        ))?;
        Ok(())
    }

    fn handle_audio_levels(
        &self,
        remote_peer_id: &str,
        captured_level: AudioLevel,
        received_level: AudioLevel,
    ) -> Result<()> {
        self.send(Event::AudioLevels {
            peer_id: remote_peer_id.to_string(),
            captured_level,
            received_level,
        })?;
        Ok(())
    }
}


impl http::Delegate for EventReporter {
    fn send_request(&self, request_id: u32, request: http::Request) {
        let _ = self.send(Event::SendHttpRequest {
            request_id,
            request,
        });
    }
}

impl GroupUpdateHandler for EventReporter {
    fn handle_group_update(&self, update: GroupUpdate) -> Result<()> {
        self.send(Event::GroupUpdate(update))?;
        Ok(())
    }
}

pub struct CallEndpoint {
    call_manager: CallManager<NativePlatform>,
    // This is what we use to control mute/not.
    // It should probably be per-call, but for now it's easier to have only one.
    outgoing_audio_track: AudioTrack,
    // This is what we use to push video frames out.
    outgoing_video_source: VideoSource,
    // We only keep this around so we can pass it to PeerConnectionFactory::create_peer_connection
    // via the NativeCallContext.
    outgoing_video_track: VideoTrack,
    // Boxed so we can pass it as a Box<dyn VideoSink>
    incoming_video_sink: Box<LastFramesVideoSink>,
    peer_connection_factory: PeerConnectionFactory,
}

impl CallEndpoint {
    fn new<'a>(
        use_new_audio_device_module: bool,
        statusCallback: extern "C" fn(CallId, u64, i32, CallMediaType),
        answerCallback: extern "C" fn(JArrayByte),
        iceUpdateCallback: extern "C" fn(JArrayByte2D),
    ) -> Result<Self> {
        // Relevant for both group calls and 1:1 calls
        let (events_sender, _events_receiver) = channel::<Event>();
        let peer_connection_factory = PeerConnectionFactory::new(pcf::Config {
            use_new_audio_device_module,
            ..Default::default()
        })?;
        let outgoing_audio_track = peer_connection_factory.create_outgoing_audio_track()?;
        outgoing_audio_track.set_enabled(false);
        let outgoing_video_source = peer_connection_factory.create_outgoing_video_source()?;
        let outgoing_video_track =
            peer_connection_factory.create_outgoing_video_track(&outgoing_video_source)?;
        outgoing_video_track.set_enabled(false);
        let incoming_video_sink = Box::new(LastFramesVideoSink::default());

        let event_reported = Arc::new(AtomicBool::new(false));

        // let event_reporter = EventReporter::new(startCallback, answerCallback, iceUpdateCallback, events_sender, move || {
        let event_reporter = EventReporter::new(statusCallback, answerCallback, iceUpdateCallback, events_sender, move || {
            info!("[JV] EVENT_REPORTER, NYI");
            if event_reported.swap(true, std::sync::atomic::Ordering::Relaxed) {
                return;
            }
        });
        // Only relevant for 1:1 calls
        let signaling_sender = Box::new(event_reporter.clone());
        let should_assume_messages_sent = false; // Use async notification from app to send next message.
        let state_handler = Box::new(event_reporter.clone());

        // Only relevant for group calls
        let http_client = http::DelegatingClient::new(event_reporter.clone());
        let group_handler = Box::new(event_reporter);

        let platform = NativePlatform::new(
            peer_connection_factory.clone(),
            signaling_sender,
            should_assume_messages_sent,
            state_handler,
            group_handler,
        );

        let call_manager = CallManager::new(platform, http_client)?;
        Ok(Self {
            call_manager,
            outgoing_audio_track,
            outgoing_video_source,
            outgoing_video_track,
            incoming_video_sink,
            peer_connection_factory,
        })
    }
}

#[derive(Clone, Default)]
struct LastFramesVideoSink {
    last_frame_by_track_id: Arc<Mutex<HashMap<u32, VideoFrame>>>,
}

impl VideoSink for LastFramesVideoSink {
    fn on_video_frame(&self, track_id: u32, frame: VideoFrame) {
        self.last_frame_by_track_id
            .lock()
            .unwrap()
            .insert(track_id, frame);
    }

    fn box_clone(&self) -> Box<dyn VideoSink> {
        Box::new(self.clone())
    }
}

impl LastFramesVideoSink {
    fn pop(&self, track_id: u32) -> Option<VideoFrame> {
        self.last_frame_by_track_id
            .lock()
            .unwrap()
            .remove(&track_id)
    }

    fn clear(&self) {
        self.last_frame_by_track_id.lock().unwrap().clear();
    }
}


#[no_mangle]
pub unsafe extern "C" fn initRingRTC(ts: JString) -> i64 {
    println!("Initialize RingRTC, init logging");
    init_logging();
    println!("Initialize RingRTC, init logging done");
println!("Ready to print {:?}", ts);
    let txt = ts.to_string();
    info!("Got text: {}", txt);
    info!("Initialized RingRTC, using logging");
    1   
}

#[no_mangle]
pub unsafe extern "C" fn getVersion() -> i64 {
    1
}

fn create_call_endpoint(audio: bool, 
            statusCallback: extern "C" fn(CallId, u64, i32, CallMediaType),
            answerCallback: extern "C" fn(JArrayByte),
            iceUpdateCallback: extern "C" fn(JArrayByte2D),
        ) -> Result<*mut CallEndpoint> {
    let call_endpoint = CallEndpoint::new(audio, statusCallback, answerCallback, iceUpdateCallback).unwrap();
    let call_endpoint_box = Box::new(call_endpoint);
    Ok(Box::into_raw(call_endpoint_box))
}

#[no_mangle]
pub unsafe extern "C" fn createCallEndpoint(statusCallback: extern "C" fn(CallId, u64, i32, CallMediaType),
            answerCallback: extern "C" fn(JArrayByte), 
            iceUpdateCallback: extern "C" fn(JArrayByte2D)) -> i64 {
    let answer: i64 = match create_call_endpoint(false, statusCallback, answerCallback, iceUpdateCallback) {
        Ok(v) => v as i64,
        Err(e) => {
            info!("Error creating callEndpoint: {}", e); 
            0
        }
    };  
    info!("[tring] CallEndpoint created at {}", answer);
    answer
}

#[no_mangle]
pub unsafe extern "C" fn setSelfUuid(endpoint: i64, ts: JString) -> i64 {
    let txt = ts.to_string();
    info!("setSelfUuid to : {}", txt);
    let uuid = txt.into_bytes();
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.set_self_uuid(uuid);
    1   
}

#[no_mangle]
pub unsafe extern "C" fn receivedOffer(endpoint: i64, peerId: JString, call_id: u64,
        offer_type:i32, sender_device_id:u32, receiver_device_id:u32,
        sender_key: JByteArray, receiver_key: JByteArray, opaque: JByteArray, age_sec: u64) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let peer_id = JString::from(peerId);
    let call_id = CallId::new(call_id);
    let call_media_type = match offer_type {
        1 => CallMediaType::Video,
        _ => CallMediaType::Audio, // TODO: Do something better.  Default matches are evil.
    };
    let offer = signaling::Offer::new(call_media_type, opaque.to_vec_u8()).unwrap();
    callendpoint.call_manager.received_offer(
            peer_id.to_string(),
            call_id,
            signaling::ReceivedOffer {
                offer,
                age: Duration::from_secs(age_sec),
                sender_device_id,
                receiver_device_id,
                // A Java desktop client cannot be the primary device.
                receiver_device_is_primary: false,
                sender_identity_key: sender_key.to_vec_u8(),
                receiver_identity_key: receiver_key.to_vec_u8(),
            },
        );


    1   
}

#[no_mangle]
pub unsafe extern "C" fn proceedCall(endpoint: i64, call_id: u64, bandwidth_mode: i32, audio_levels_interval_millis:i32) -> i64 {
    info!("Proceeding with call");
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let call_id = CallId::from(call_id);
    let ice_server = IceServer::new(String::from("iceuser"), String::from("icepwd"), Vec::new());
    let context = NativeCallContext::new(
        false,
        ice_server,
        endpoint.outgoing_audio_track.clone(),
        endpoint.outgoing_video_track.clone(),
        endpoint.incoming_video_sink.clone(),
    );
    let audio_levels_interval = if audio_levels_interval_millis <= 0 {
        None
    } else {
        Some(Duration::from_millis(audio_levels_interval_millis as u64))
    };
    endpoint.call_manager.proceed(
        call_id,
        context,
        BandwidthMode::from_i32(bandwidth_mode),
        audio_levels_interval);

    147
}

#[no_mangle]
pub unsafe extern "C" fn receivedIce(endpoint: i64, call_id: u64, sender_device_id: DeviceId, icepack: JByteArray2D) {
    info!("JavaRing, received_ice with length = {}", icepack.len );
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Received offer, endpoint = {:?}", endpoint);
    let call_id = CallId::from(call_id);
    let mut ice_candidates = Vec::new();
    for j in 0..icepack.len {
        let row = &icepack.buff[j];
        let opaque = row.to_vec_u8();
        ice_candidates.push(signaling::IceCandidate::new(opaque));
    }   
    callendpoint.call_manager.received_ice(
        call_id,
        signaling::ReceivedIce {
            ice: signaling::Ice {
                candidates: ice_candidates,
            },
            sender_device_id,
        },
    );
}

#[no_mangle]
pub unsafe extern "C" fn acceptCall(endpoint: i64, call_id: u64) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("now accept call");
    let call_id = CallId::from(call_id);
    endpoint.call_manager.accept_call(call_id);
    573 
}

#[no_mangle]
pub unsafe extern "C" fn signalMessageSent(endpoint: i64, call_id: CallId) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Received signalmessagesent, endpoint = {:?}", endpoint);
    callendpoint.call_manager.message_sent(call_id);
    135 
}

#[no_mangle]
pub unsafe extern "C" fn setAudioInput(endpoint: i64, index: u16) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Have to set audio_recordig_device to {}", index);
    endpoint.peer_connection_factory.set_audio_recording_device(index);
    1
}

#[no_mangle]
pub unsafe extern "C" fn setOutgoingAudioEnabled(endpoint: i64, enable: bool) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Have to set outgoing audio enabled to {}", enable);
    endpoint.outgoing_audio_track.set_enabled(enable);
    1
}


