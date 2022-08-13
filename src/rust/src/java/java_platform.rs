use std::collections::{HashSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use crate::common::{ApplicationEvent, CallDirection, CallId, CallMediaType, DeviceId, Result};
use crate::core::bandwidth_mode::BandwidthMode;
use crate::core::call::Call;
use crate::core::connection::{Connection, ConnectionType};
use crate::core::platform::{Platform, PlatformItem};
use crate::core::{group_call, signaling};
use crate::lite::{
    http, sfu,
    sfu::{DemuxId, PeekInfo, PeekResult, UserId},
};

use crate::webrtc;
use crate::webrtc::media::{AudioTrack, MediaStream, VideoFrame, VideoSink, VideoTrack};
use crate::webrtc::peer_connection::{AudioLevel, ReceivedAudioLevel};
use crate::webrtc::peer_connection_observer::{NetworkRoute, PeerConnectionObserver};
use crate::webrtc::peer_connection_factory::{self as pcf, IceServer, PeerConnectionFactory};
use crate::webrtc::peer_connection::RffiPeerConnection;

pub type PeerId = String;

struct JavaConnection {
    platform: JavaPlatform,
    /// Java Connection object.
    jni_connection: i64,
}

#[derive(Clone)]
pub struct JDKConnection {
    inner: Arc<JavaConnection>,
}

unsafe impl Sync for JDKConnection {}
unsafe impl Send for JDKConnection {}
impl PlatformItem for JDKConnection {}

impl JDKConnection {
    fn new(platform: JavaPlatform, jni_connection: i64) -> Self {
        Self {
            inner: Arc::new(JavaConnection {
                platform,
                jni_connection,
            }),
        }
    }

    pub fn to_jni(&self) -> i64 {
        self.inner.jni_connection.clone()
    }
}


extern "C" {
    pub fn Rust_borrowPeerConnectionFromJniOwnedPeerConnection(
        jni_owned_pc: i64,
    ) -> webrtc::ptr::BorrowedRc<RffiPeerConnection>;
}
// pub type JavaCallContext = String;

#[derive(Clone)]
pub struct JavaCallContext {
    hide_ip: bool,
    ice_server: IceServer,
    outgoing_audio_track: AudioTrack,
    outgoing_video_track: VideoTrack,
    incoming_video_sink: Box<dyn VideoSink>,
}

impl JavaCallContext {
    pub fn new(
        hide_ip: bool,
        ice_server: IceServer,
        outgoing_audio_track: AudioTrack,
        outgoing_video_track: VideoTrack,
        incoming_video_sink: Box<dyn VideoSink>,
    ) -> Self {
        Self {
            hide_ip,
            ice_server,
            outgoing_audio_track,
            outgoing_video_track,
            incoming_video_sink,
        }
    }
}

impl PlatformItem for JavaCallContext {}

pub struct JavaMediaStream {
}

impl JavaMediaStream {
    pub fn new(_incoming_media: MediaStream) -> Self {
        Self {}
    }
}

impl PlatformItem for JavaMediaStream {
}

#[allow(non_snake_case)]
extern "C" fn dummyStart(call_id: CallId, remote_peer: u64, direction: CallDirection, call_media_type: CallMediaType) {
    info!("Dummy start with {:?}", (call_id, remote_peer, direction, call_media_type));
}

#[allow(non_snake_case)]
extern "C" fn dummyCreateConnection(_ptr: u64, call_id: CallId) -> i64 {
    info!("Dummy createConnection for {:?}", call_id);
    123456
}

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Clone)]
pub struct JavaPlatform {
#[allow(non_snake_case)]
    pub startCallback: unsafe extern "C" fn(call_id: CallId,
                                            remote_peer: u64,
                                            direction: CallDirection,
                                            call_media_type: CallMediaType),
    pub createConnectionCallback: unsafe extern "C" fn(connection_ptr: u64, call_id: CallId) -> i64,
    pub peer_connection_factory: PeerConnectionFactory,
    pub context: JavaCallContext,
    pub bogusVal: i32
}

impl JavaPlatform {
    pub fn new() -> Result<Self> {
        info!("JavaPlatform created!");
        let myfactory = PeerConnectionFactory::new(pcf::Config {..Default::default()})?;
        let outgoing_audio_track = myfactory.create_outgoing_audio_track()?;
        outgoing_audio_track.set_enabled(false);
        let outgoing_video_source = myfactory.create_outgoing_video_source()?;
        let outgoing_video_track =
            myfactory.create_outgoing_video_track(&outgoing_video_source)?;
        outgoing_video_track.set_enabled(false);
        let incoming_video_sink = Box::new(LastFramesVideoSink::default());
        let ice_server = IceServer::new(String::from("iceuser"), String::from("icepwd"), Vec::new());

        Ok(Self {
            startCallback : dummyStart,
            createConnectionCallback : dummyCreateConnection,
            peer_connection_factory: myfactory,
            context : JavaCallContext::new(false, ice_server, outgoing_audio_track, outgoing_video_track, incoming_video_sink),
            bogusVal: 12
        })
    }

/*
    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self {
            startCallback : self.startCallback,
            createConnectionCallback : dummyCreateConnection,
            peer_connection_factory: PeerConnectionFactory::new(pcf::Config {..Default::default()})?,
            context: self.context,
            bogusVal: 15
        })
    }
*/

    #[no_mangle]
    pub unsafe extern "C" fn setStartCallCallback(&mut self, func: unsafe extern "C" fn(CallId, u64, CallDirection, CallMediaType)) {
        self.startCallback = func;
    }

}
#[derive(Clone, Default)]
pub struct LastFramesVideoSink {
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


impl http::Delegate for JavaPlatform {
    fn send_request(&self, _request_id: u32, _request: http::Request) {
        info!("JavaPlatform.send_http_request NOT IMPLEMENTED");
        // if let Err(err) = self.send_http_request(request_id, request) {
       // error!("JavaPlatform.send_http_request failed: {:?}", err);
        // }
    }
}

impl Platform for JavaPlatform {
    type AppRemotePeer = PeerId; 
    type AppCallContext = JavaCallContext;
    type AppConnection = JDKConnection;
    type AppIncomingMedia = JavaMediaStream;

    fn compare_remotes(
        &self,
        remote_peer1: &Self::AppRemotePeer,
        remote_peer2: &Self::AppRemotePeer,
    ) -> Result<bool> {
        info!(
            "NativePlatform::compare_remotes(): remote1: {}, remote2: {}",
            remote_peer1, remote_peer2
        );

        Ok(remote_peer1 == remote_peer2)
    }

    fn create_connection(
        &mut self,
        call: &Call<Self>,
        remote_device_id: DeviceId,
        connection_type: ConnectionType,
        signaling_version: signaling::Version,
        bandwidth_mode: BandwidthMode,
        audio_levels_interval: Option<Duration>,
    ) -> Result<Connection<Self>> {
        info!(
            "JavaPlatform::create_connection(): call: {} remote_device_id: {} signaling_version: {:?}",
            call, remote_device_id, signaling_version
        );
        let connection = Connection::new(
            call.clone(),
            remote_device_id,
            connection_type,
            bandwidth_mode,
            audio_levels_interval,
            None,
        )?;
        if (1 > 2) {
            let connection_ptr = connection.get_connection_ptr()?;
            info!("[javaplatform] Connection_ptr = {:?}", connection_ptr);
            let call_id = call.call_id();
            info!("TODO: Create Connection in Java layer (similar to Android CallManager.createConnection)");
            let java_owned_pc = unsafe {
                (self.createConnectionCallback)(connection_ptr.as_ptr() as u64, call_id)
            };
            info!("DID callback to Java to create connection pointer");
            info!("DID call cccallback, java_owned_pc = {:?}", java_owned_pc);
            // let platform = self.try_clone()?;
            // let jdk_connection = JDKConnection::new(platform, java_owned_pc);
            info!("Done with create_connection!");
        } else {
            let context = call.call_context()?;
            let pc_observer = PeerConnectionObserver::new(
                connection.get_connection_ptr()?,
                false, /* enable_frame_encryption */
                true,  /* enable_video_frame_event */
            )?;
            let pc = self.peer_connection_factory.create_peer_connection(
                pc_observer,
                context.hide_ip,
                &context.ice_server,
                context.outgoing_audio_track.clone(),
                Some(context.outgoing_video_track.clone()),
            )?;

            connection.set_peer_connection(pc)?;
        }

        Ok(connection)
    }

    fn on_start_call(
        &self,
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId, 
        direction: CallDirection,
        call_media_type: CallMediaType,
    ) -> Result<()> {
        info!("on_start_call(): call_id: {:?}, remote_peer: {:?}, direction: {}",
            call_id, remote_peer, direction);
        info!("Current thread = {:?}", std::thread::current().id());
        unsafe {
            info!("Ready to call callback");
            info!("Ready to call callback for {:?}",self);
            info!("Ready to call callback for {}",self.bogusVal);
            // info!("Ready to call callback at {:?}",self.startCallback);
            // myCallback(39);
            let pid = 123;
            (self.startCallback)(call_id, pid, direction, call_media_type);
            info!("DID call callback");
        }
        Ok(())
    }

    fn on_event(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        _call_id: CallId, 
        event: ApplicationEvent,
    ) -> Result<()> {
        info!("on_event(): {}", event); 
        Ok(())
    }

     fn on_network_route_changed(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        network_route: NetworkRoute,
    ) -> Result<()> {
        info!(
            "on_network_route_changed(): network_route: {:?}",
            network_route
        );
        Ok(())
    }

    fn on_audio_levels(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        captured_level: AudioLevel,
        received_level: AudioLevel,
    ) -> Result<()> {
        trace!(
            "on_audio_levels(): captured_level: {}; received_level: {}",
            captured_level,
            received_level
        );
        Ok(())
    }

    fn on_send_offer(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        _offer: signaling::Offer,
    ) -> Result<()> {
        info!("on_send_offer(): call_id: {}", call_id);
        Ok(())
    }

    fn on_send_answer(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        _send: signaling::SendAnswer,
    ) -> Result<()> {
        info!(
            "on_send_answer(): call_id: {}",
            call_id
        );
        Ok(())
    }


    fn on_send_ice(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        _send: signaling::SendIce,
    ) -> Result<()> {
        info!(
            "on_send_ice(): call_id: {}",
            call_id
        );
        Ok(())
    }

    fn on_send_hangup(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        _send: signaling::SendHangup,
    ) -> Result<()> {
        info!("on_send_hangup(): call_id: {}", call_id);
        Ok(())
    }

    fn on_send_busy(&self, _remote_peer: &Self::AppRemotePeer, call_id: CallId) -> Result<()> {
        info!("on_send_busy(): call_id: {}", call_id);
        Ok(())
    }

    fn send_call_message(
        &self,
        _recipient_uuid: Vec<u8>,
        _message: Vec<u8>,
        _urgency: group_call::SignalingMessageUrgency,
    ) -> Result<()> {
        info!("send_call_message():");
        Ok(())
    }

    fn send_call_message_to_group(
        &self,
        _group_id: Vec<u8>,
        _message: Vec<u8>,
        _urgency: group_call::SignalingMessageUrgency,
    ) -> Result<()> {
        info!("send_call_message_to_group():");
        Ok(())
    }

    fn create_incoming_media(
        &self,
        _connection: &Connection<Self>,
        incoming_media: MediaStream,
    ) -> Result<Self::AppIncomingMedia> {
        Ok(JavaMediaStream::new(incoming_media))
    }

    fn connect_incoming_media(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        _app_call_context: &Self::AppCallContext,
        _incoming_media: &Self::AppIncomingMedia,
    ) -> Result<()> {
        info!("connect_incoming_media():");
        Ok(())
    }

    fn disconnect_incoming_media(&self, _app_call_context: &Self::AppCallContext) -> Result<()> {
        info!("disconnect_incoming_media():");
        Ok(())
    }

    /// Notify the application that an offer is too old.
    fn on_offer_expired(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        _call_id: CallId, 
        _age: Duration,
    ) -> Result<()> {
        info!("NOT IMPLEMENTED");
        Ok(())
    }

    /// Notify the application that the call is completely concluded
    fn on_call_concluded(&self, _remote_peer: &Self::AppRemotePeer, _call_id: CallId) -> Result<()> {
        info!("NOT IMPLEMENTED");
        Ok(())
    }

    fn group_call_ring_update(
        &self,
        _group_id: group_call::GroupId,
        _ring_id: group_call::RingId,
        _sender: UserId,
        _update: group_call::RingUpdate,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn request_membership_proof(&self, _client_id: group_call::ClientId) {
        info!("NOT IMPLEMENTED")
    }

    fn request_group_members(&self, _client_id: group_call::ClientId) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_connection_state_changed(
        &self,
        _client_id: group_call::ClientId,
        _connection_state: group_call::ConnectionState,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_network_route_changed(
        &self,
        _client_id: group_call::ClientId,
        _network_route: NetworkRoute,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_join_state_changed(
        &self,
        _client_id: group_call::ClientId,
        _join_state: group_call::JoinState,
    ) {
        info!("NOT IMPLEMENTED")
    }
    fn handle_remote_devices_changed(
        &self,
        _client_id: group_call::ClientId,
        _remote_device_states: &[group_call::RemoteDeviceState],
        _reason: group_call::RemoteDevicesChangedReason,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_incoming_video_track(
        &self,
        _client_id: group_call::ClientId,
        _remote_demux_id: DemuxId,
        _incoming_video_track: VideoTrack,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_peek_changed(
        &self,
        _client_id: group_call::ClientId,
        _peek_info: &PeekInfo,
        _joined_members: &HashSet<UserId>,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_audio_levels(
        &self,
        _client_id: group_call::ClientId,
        _captured_level: AudioLevel,
        _received_levels: Vec<ReceivedAudioLevel>,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_ended(&self, _client_id: group_call::ClientId, _reason: group_call::EndReason) {
        info!("NOT IMPLEMENTED")
    }
}

impl fmt::Display for JavaPlatform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "JavaPlatform")
    }
}

impl fmt::Debug for JavaPlatform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl sfu::Delegate for JavaPlatform {
    fn handle_peek_result(&self, _request_id: u32, _peek_result: PeekResult) {
        info!("JavaPlatform::NYIhandle_peek_result(): id: {}", _request_id);
    }



}
/*
    fn embedded_create_connection(connection: Connection, call: Call) {
    }

    // similar to native.rs
    fn native_create_connection(connection : Connection) {

    }
*/


