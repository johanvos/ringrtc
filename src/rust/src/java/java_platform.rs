use std::collections::HashSet;
use std::fmt;
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

use crate::webrtc::media::{MediaStream, VideoTrack};
use crate::webrtc::peer_connection::{AudioLevel, ReceivedAudioLevel};
use crate::webrtc::peer_connection_observer::NetworkRoute;

#[derive(Clone)]
#[repr(C)]
pub struct PeerId {
    pub address: u64
}

impl PeerId {
    pub fn new() -> Self {
        Self {
            address: 0
        }
    }
}

impl PlatformItem for PeerId {}

pub type JavaCallContext = String;
pub type JavaConnection = String;

pub struct JavaMediaStream {
}

impl JavaMediaStream {
    pub fn new(_incoming_media: MediaStream) -> Self {
        Self {}
    }
}

impl PlatformItem for JavaMediaStream {
}

#[repr(C)]
pub struct JavaPlatform {
}


impl JavaPlatform {
    pub fn new() -> Self {
        Self {}
    }

    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self {
        })
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
impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PeerId")
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl Platform for JavaPlatform {
    type AppRemotePeer = PeerId; 
    type AppCallContext = JavaCallContext;
    type AppConnection = JavaConnection;
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

        Ok(remote_peer1.address == remote_peer2.address)
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

        Ok(connection)
    }

    fn on_start_call(
        &self,
        _remote_peer: &Self::AppRemotePeer,
        call_id: CallId, 
        direction: CallDirection,
        _call_media_type: CallMediaType,
    ) -> Result<()> {
        info!(
            "on_start_call(): call_id: {}, direction: {}",
            call_id, direction
        );
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
