#![allow(unused_parens)]

use std::collections::HashMap;

use std::slice;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libc::size_t;

use crate::common::{CallDirection, CallId, CallMediaType, DeviceId, Result};
use crate::core::bandwidth_mode::BandwidthMode;
use crate::core::call_manager::CallManager;
use crate::core::group_call;
use crate::core::group_call::{GroupId, SignalingMessageUrgency};
use crate::core::signaling;
use crate::core::util::ptr_as_mut;

use crate::java::jtypes::{
    JArrayByte, JArrayByte2D, JByteArray, JByteArray2D, JString, TringDevice,
};

use crate::lite::http;
use crate::lite::sfu::UserId;
use crate::native::{
    CallState, CallStateHandler, GroupUpdate, GroupUpdateHandler, NativeCallContext,
    NativePlatform, PeerId, SignalingSender,
};
use crate::webrtc::logging;
use crate::webrtc::media::{
    AudioTrack, VideoFrame, VideoPixelFormat, VideoSink, VideoSource, VideoTrack,
};

use crate::webrtc::peer_connection::AudioLevel;

use crate::webrtc::peer_connection_factory::{
    self as pcf, AudioDevice, IceServer, PeerConnectionFactory,
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
    pub statusCallback: unsafe extern "C" fn(u64, u64, i32, i32),
    pub answerCallback: unsafe extern "C" fn(JArrayByte),
    pub offerCallback: unsafe extern "C" fn(JArrayByte),
    pub iceUpdateCallback: unsafe extern "C" fn(JArrayByte),
    sender: Sender<Event>,
    report: Arc<dyn Fn() + Send + Sync>,
}

impl EventReporter {
    fn new(
        statusCallback: extern "C" fn(u64, u64, i32, i32),
        answerCallback: extern "C" fn(JArrayByte),
        offerCallback: extern "C" fn(JArrayByte),
        iceUpdateCallback: extern "C" fn(JArrayByte),
        sender: Sender<Event>,
        report: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            statusCallback,
            answerCallback,
            offerCallback,
            iceUpdateCallback,
            sender,
            report: Arc::new(report),
        }
    }

    fn send(&self, event: Event) -> Result<()> {
        match event {
            Event::SendSignaling(_peer_id, _maybe_device_id, call_id, signal) => {
                info!("JavaPlatform needs to send SignalingEvent to app");
                match signal {
                    signaling::Message::Offer(offer) => {
                        info!(
                            "[JV] SendSignaling OFFER Event and call_id = {}",
                            call_id.as_u64()
                        );
                        let op = JArrayByte::new(offer.opaque);
                        unsafe {
                            (self.offerCallback)(op);
                        }
                    }
                    signaling::Message::Answer(answer) => {
                        info!("[JV] SendSignaling ANSWER Event");
                        let op = JArrayByte::new(answer.opaque);
                        unsafe {
                            (self.answerCallback)(op);
                        }
                    }
                    signaling::Message::Ice(ice) => {
                        info!("[JV] SendSignaling ICE Event");
                        let ilen = ice.candidates.len();
                        unsafe {
                            for i in 0..ilen {
                                (self.iceUpdateCallback)(JArrayByte::new(
                                    ice.candidates[i].opaque.clone(),
                                ));
                            }
                        }
                    }
                    signaling::Message::Hangup(hangup) => {
                        let (hangup_type, hangup_device_id) = hangup.to_type_and_device_id();
                        let device_id: u64 = match hangup_device_id {
                            Some(device_id) => device_id.into(),
                            None => 0,
                        };
                        info!("[JV] SendSignaling Hangup Event");
                        unsafe {
                            (self.statusCallback)(
                                call_id.as_u64(),
                                device_id,
                                11,
                                hangup_type as i32,
                            );
                        }
                    }
                    _ => {
                        info!("[JV] unknownSendSignalingEvent WHICH IS WHAT WE NEED TO FIX NOW!");
                    }
                }
                info!("JavaPlatform asked app to send SignalingEvent");
            }
            Event::CallState(_peer_id, call_id, CallState::Incoming(call_media_type)) => {
                info!("[JV] CALLSTATEEVEMNT");
                let direction = 0;
                unsafe {
                    (self.statusCallback)(call_id.as_u64(), 1, direction, call_media_type as i32);
                }
            }
            Event::CallState(_peer_id, call_id, CallState::Outgoing(call_media_type)) => {
                info!("[JV] CALLSTATEEVEMNT");
                let direction = 1;
                unsafe {
                    (self.statusCallback)(call_id.as_u64(), 1, direction, call_media_type as i32);
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
                    (self.statusCallback)(call_id.as_u64(), 1, 10 * state_index, 0);
                }
            }
            Event::RemoteVideoStateChange(peer_id, enabled) => {
                info!("RemoveVideoStateChange to {}", enabled);
                unsafe {
                    if enabled {
                        (self.statusCallback)(1, 1, 22, 31);
                    } else {
                        (self.statusCallback)(1, 1, 22, 32);
                    }
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
    videoFrameCallback: extern "C" fn(*const u8, u32, u32, size_t),
}

impl CallEndpoint {
    fn new<'a>(
        use_new_audio_device_module: bool,
        statusCallback: extern "C" fn(u64, u64, i32, i32),
        answerCallback: extern "C" fn(JArrayByte),
        offerCallback: extern "C" fn(JArrayByte),
        iceUpdateCallback: extern "C" fn(JArrayByte),
        videoFrameCallback: extern "C" fn(*const u8, u32, u32, size_t),
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

        let event_reporter = EventReporter::new(
            statusCallback,
            answerCallback,
            offerCallback,
            iceUpdateCallback,
            events_sender,
            move || {
                info!("[JV] EVENT_REPORTER, NYI");
                if event_reported.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            },
        );
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
            videoFrameCallback,
        })
    }
}

#[derive(Clone, Default)]
struct LastFramesVideoSink {
    last_frame_by_track_id: Arc<Mutex<HashMap<u32, VideoFrame>>>,
}

impl VideoSink for LastFramesVideoSink {
    fn on_video_frame(&self, track_id: u32, frame: VideoFrame) {
        info!("Got videoframe!");
        // let myframe: &mut[u8;512] = &mut [0;512];
        // frame.to_rgba(myframe.as_mut_slice());
        // info!("uploading frame = {:?}", myframe);
        // info!("frame uploaded");
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

#[no_mangle]
pub unsafe extern "C" fn createCallEndpoint(
    statusCallback: extern "C" fn(u64, u64, i32, i32),
    answerCallback: extern "C" fn(JArrayByte),
    offerCallback: extern "C" fn(JArrayByte),
    iceUpdateCallback: extern "C" fn(JArrayByte),
    videoFrameCallback: extern "C" fn(*const u8, u32, u32, size_t),
) -> i64 {
    let call_endpoint = CallEndpoint::new(
        false,
        statusCallback,
        answerCallback,
        offerCallback,
        iceUpdateCallback,
        videoFrameCallback,
    )
    .unwrap();
    let call_endpoint_box = Box::new(call_endpoint);
    let boxx: Result<*mut CallEndpoint> = Ok(Box::into_raw(call_endpoint_box));

    let answer: i64 = match boxx {
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
pub unsafe extern "C" fn receivedOffer(
    endpoint: i64,
    peerId: JString,
    call_id: u64,
    offer_type: i32,
    sender_device_id: u32,
    receiver_device_id: u32,
    sender_key: JByteArray,
    receiver_key: JByteArray,
    opaque: JByteArray,
    age_sec: u64,
) -> i64 {
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
pub unsafe extern "C" fn receivedAnswer(
    endpoint: i64,
    peerId: JString,
    call_id: u64,
    sender_device_id: u32,
    sender_key: JByteArray,
    receiver_key: JByteArray,
    opaque: JByteArray,
) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let peer_id = JString::from(peerId);
    let call_id = CallId::new(call_id);
    let answer = signaling::Answer::new(opaque.to_vec_u8()).unwrap();
    callendpoint.call_manager.received_answer(
        call_id,
        signaling::ReceivedAnswer {
            answer,
            sender_device_id,
            sender_identity_key: sender_key.to_vec_u8(),
            receiver_identity_key: receiver_key.to_vec_u8(),
        },
    );
    1
}

// suppy a random callid
#[no_mangle]
pub unsafe extern "C" fn createOutgoingCall(
    endpoint: i64,
    peer_id: JString,
    video_enabled: bool,
    local_device_id: u32,
    call_id: i64,
) -> i64 {
    info!("create outgoing call");
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let peer_id = peer_id.to_string();
    let media_type = if video_enabled {
        CallMediaType::Video
    } else {
        CallMediaType::Audio
    };
    let call_id = CallId::from(call_id);
    endpoint
        .call_manager
        .create_outgoing_call(peer_id, call_id, media_type, local_device_id);
    1
}

#[no_mangle]
pub unsafe extern "C" fn proceedCall(
    endpoint: i64,
    call_id: u64,
    bandwidth_mode: i32,
    audio_levels_interval_millis: i32,
    ice_user: JString,
    ice_pwd: JString,
    icepack: JByteArray2D,
) -> i64 {
    info!("Proceeding with call");
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let call_id = CallId::from(call_id);
    let mut ice_candidates = Vec::new();
    for j in 0..icepack.len {
        let row = &icepack.buff[j];
        let opaque = row.to_vec_u8();
        ice_candidates.push(String::from_utf8(opaque).unwrap());
    }
    let ice_server = IceServer::new(ice_user.to_string(), ice_pwd.to_string(), ice_candidates);
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
        audio_levels_interval,
    );

    147
}

#[no_mangle]
pub unsafe extern "C" fn receivedIce(
    endpoint: i64,
    call_id: u64,
    sender_device_id: DeviceId,
    icepack: JByteArray2D,
) {
    info!("receivedIce from app with length = {}", icepack.len);
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
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
    info!("receivedIce invoked call_manager and will now return to app");
}

#[no_mangle]
pub unsafe extern "C" fn acceptCall(endpoint: i64, call_id: u64) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("acceptCall requested by app");
    let call_id = CallId::from(call_id);
    endpoint.call_manager.accept_call(call_id);
    573
}

#[no_mangle]
pub unsafe extern "C" fn ignoreCall(endpoint: i64, call_id: u64) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("now drop (ignore) call");
    let call_id = CallId::from(call_id);
    endpoint.call_manager.drop_call(call_id);
    1
}

#[no_mangle]
pub unsafe extern "C" fn hangupCall(endpoint: i64) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("now hangup call");
    endpoint.call_manager.hangup();
    1
}

#[no_mangle]
pub unsafe extern "C" fn signalMessageSent(endpoint: i64, call_id: CallId) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Received signalmessagesent, endpoint = {:?}", endpoint);
    callendpoint.call_manager.message_sent(call_id);
    135
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn getAudioInputs(endpoint: i64, idx: u32) -> TringDevice<'static> {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let devices = callendpoint
        .peer_connection_factory
        .get_audio_recording_devices()
        .unwrap();
    // let mut answer: [TringDevice;16] = [TringDevice::empty();16];
    let mut answer: TringDevice = TringDevice::empty();
    for (i, device) in devices.iter().enumerate() {
        let wd = TringDevice::from_fields(
            i as u32,
            device.name.clone(),
            device.unique_id.clone(),
            device.i18n_key.clone(),
        );
        if (i as u32 == idx) {
            answer = wd;
        }
        // answer[i] = wd;
    }
    answer
}

#[no_mangle]
pub unsafe extern "C" fn setAudioInput(endpoint: i64, index: u16) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Have to set audio_recordig_device to {}", index);
    endpoint
        .peer_connection_factory
        .set_audio_recording_device(index);
    1
}

#[no_mangle]
pub unsafe extern "C" fn getAudioOutputs(endpoint: i64) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let devices = callendpoint
        .peer_connection_factory
        .get_audio_playout_devices();

    for device in devices.iter() {
        info!("OUTDEVICE = {:#?}", device);
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn setAudioOutput(endpoint: i64, index: u16) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Have to set audio_output_device to {}", index);
    endpoint
        .peer_connection_factory
        .set_audio_playout_device(index);
    1
}

#[no_mangle]
pub unsafe extern "C" fn setOutgoingAudioEnabled(endpoint: i64, enable: bool) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Have to set outgoing audio enabled to {}", enable);
    endpoint.outgoing_audio_track.set_enabled(enable);
    1
}

#[no_mangle]
pub unsafe extern "C" fn setOutgoingVideoEnabled(endpoint: i64, enable: bool) -> i64 {
    info!("Hava to setOutgoingVideoEnabled({})", enable);
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    endpoint.outgoing_video_track.set_enabled(enable);
    let mut active_connection = endpoint.call_manager.active_connection();
    if (active_connection.is_ok()) {
        active_connection
            .expect("No active connection!")
            .update_sender_status(signaling::SenderStatus {
                video_enabled: Some(enable),
                ..Default::default()
            });
    } else {
        info!("No active connection")
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn sendVideoFrame(
    endpoint: i64,
    width: u32,
    height: u32,
    pixel_format: i32,
    raw: *const u8,
) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let mut size = width * height * 2;
    if (pixel_format == 1) {
        size = size * 2;
    }
    info!(
        "Will send VideoFrame, width = {}, heigth = {}, pixelformat = {}, size = {}",
        width, height, pixel_format, size
    );
    let buffer: &[u8] = unsafe { slice::from_raw_parts(raw, size as usize) };

    let pixel_format = VideoPixelFormat::from_i32(pixel_format);
    let pixel_format = pixel_format.unwrap();
    info!(
        "buf[0] = {} and buf[1] = {} and  buf[300] = {}, size = {}",
        buffer[0], buffer[1], buffer[300], size
    );
    let frame = VideoFrame::copy_from_slice(width, height, pixel_format, buffer);
    endpoint.outgoing_video_source.push_frame(frame);
    1
}

#[no_mangle]
pub unsafe extern "C" fn retrieveRemoteVideoFrame(endpoint: i64) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Have to retrieve remote video frame");
    let frame = endpoint.incoming_video_sink.pop(0);
    if let Some(frame) = frame {
        let frame = frame.apply_rotation();
        let width: u32 = frame.width();
        let height: u32 = frame.height();
        let myframe: &mut [u8] = &mut [0; 512000];
        frame.to_rgba(myframe);
        info!(
            "Frame0 = {}, w = {}, h = {}",
            myframe[0],
            frame.width(),
            frame.height()
        );
        let o1 = Box::new(myframe);
        let op = o1.as_ptr();
        (endpoint.videoFrameCallback)(op, width, height, 512000);
        1
    } else {
        0
    }
}
