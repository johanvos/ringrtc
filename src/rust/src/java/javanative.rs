use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common::{CallId, CallMediaType, DeviceId, Result};
use crate::core::bandwidth_mode::BandwidthMode;
use crate::core::call_manager::CallManager;
use crate::core::group_call;
use crate::core::group_call::{GroupId, SignalingMessageUrgency};
use crate::core::signaling;
use crate::core::util::{ptr_as_mut, ptr_as_box};

use crate::java::java::{MyKey, Opaque};

use crate::lite::{
    http,
    sfu::{DemuxId, GroupMember, PeekInfo, UserId},
};
use crate::native::{
    CallState, CallStateHandler, EndReason, GroupUpdate, GroupUpdateHandler, NativeCallContext,
    NativePlatform, PeerId, SignalingSender,
};
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
    info!("INFO logging enabled");
}

lazy_static! {
    static ref CURRENT_EVENT_REPORTER: Mutex<Option<EventReporter>> = Mutex::new(None);
}

// When JavaScript processes events, we want everything to go through a common queue that
// combines all the things we want to "push" to it.
// (Well, everything except log messages.  See above as to why).
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
struct EventReporter {
    sender: Sender<Event>,
    report: Arc<dyn Fn() + Send + Sync>,
}

impl EventReporter {
    fn new(sender: Sender<Event>, report: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            sender,
            report: Arc::new(report),
        }
    }

    fn send(&self, event: Event) -> Result<()> {
        self.sender.send(event)?;
        self.report();
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
    outgoing_audio_track: AudioTrack,
    outgoing_video_source: VideoSource,
    outgoing_video_track: VideoTrack,
    incoming_video_sink: Box<LastFramesVideoSink>,
    peer_connection_factory: PeerConnectionFactory,
}

impl CallEndpoint {
    fn new(
        use_new_audio_device_module: bool,
    ) -> Result<Self> {
        // Relevant for both group calls and 1:1 calls
        let (events_sender, events_receiver) = channel::<Event>();
        info!("Creating peer_connection_factory");
        let peer_connection_factory = PeerConnectionFactory::new(pcf::Config {
            use_new_audio_device_module,
            ..Default::default()
        })?;
        info!("Got peer_connection_factory: {:?}", peer_connection_factory);
        let outgoing_audio_track = peer_connection_factory.create_outgoing_audio_track()?;
        outgoing_audio_track.set_enabled(false);
        let outgoing_video_source = peer_connection_factory.create_outgoing_video_source()?;
        // if 2 < 3 {
            info!("Creating video track, this might fail if there is no camera");
            let outgoing_video_track = peer_connection_factory.create_outgoing_video_track(&outgoing_video_source)?;
            // outgoing_video_track.set_enabled(false);
        // } else {
            // info!("NOT creating video track, see javanative.rs");
        // }

        let incoming_video_sink = Box::new(LastFramesVideoSink::default());

        let event_reported = Arc::new(AtomicBool::new(false));

        let event_reporter = EventReporter::new(events_sender, move || {
            // First check to see if an event has been reported recently.
            // We aren't using this for synchronizing any other memory state,
            // so Relaxed is good enough.
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

        info!("platform = {:?}", platform);
        let call_manager = CallManager::new(platform, http_client)?;
        info!("CallEndpoint created.");
        info!("pcf = {:?}", peer_connection_factory);
        info!("call_manager = {:?}", call_manager);

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
pub unsafe extern "C" fn initRingRTC() -> i64 {
    println!("Initialize RingRTC, init logging");
    init_logging();
    info!("Initialized RingRTC, using logging");
    1
}

fn create_call_endpoint(audio: bool) -> Result<*mut CallEndpoint> {
    let call_endpoint = CallEndpoint::new(audio).unwrap();
    let call_endpoint_box = Box::new(call_endpoint);
    Ok(Box::into_raw(call_endpoint_box))
}
#[no_mangle]
pub unsafe extern "C" fn createCallEndpoint() -> i64 {
    info!("Creating CallEndpoint");
    let answer: i64 = match create_call_endpoint(false) {
        Ok(v) => v as i64,
        Err(e) => {
            info!("Error creating callEndpoint: {}", e);
            0
        }
    };
    info!("[JV] endpoint created at {}", answer);
    answer
}

#[no_mangle]
pub unsafe extern "C" fn receivedOffer(endpoint: i64, call_id: CallId,
        call_media_type: CallMediaType,
        sender_device_id: DeviceId,
        receiver_device_id: DeviceId,
        sender_identity_key: MyKey,
        receiver_identity_key: MyKey,
        opaque: Opaque,
        age_sec: u64) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("Received offer, endpoint = {:?}", endpoint);
    let peer_id = String::from("MYPEER");
    let opvec = opaque.data.to_vec();
    let opvec2 = opvec[0..opaque.len].to_vec();
    let receiver_identity_key = receiver_identity_key.data.to_vec();
    let sender_identity_key = sender_identity_key.data.to_vec();
    info!("Now create offer. cmt = {:?}, oplen = {}", call_media_type,  opaque.len);
    let offer = signaling::Offer::new(call_media_type, opvec2).unwrap();
    info!("Created offer.");

    callendpoint.call_manager.received_offer(
            peer_id,
            call_id,
            signaling::ReceivedOffer {
                offer,
                age: Duration::from_secs(age_sec),
                sender_device_id,
                receiver_device_id,
                // A Java desktop client cannot be the primary device.
                receiver_device_is_primary: false,
                sender_identity_key,
                receiver_identity_key,
            },
        );

    2
}

#[no_mangle]
pub unsafe extern "C" fn proceedCall(endpoint: i64, call_id: CallId, bandwidth_mode: i32, audio_levels_interval_millis:i32) -> i64 {
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
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

