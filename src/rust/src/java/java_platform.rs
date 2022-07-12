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


pub type PeerId = String;
pub type JavaCallContext = String;
pub type JavaConnection = String;

pub struct JavaMediaStream {
}

impl JavaMediaStream {
    pub fn new(incoming_media: MediaStream) -> Self {
        Self {}
    }
}

impl PlatformItem for JavaMediaStream {
}
impl PlatformItem for PeerId {}

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
    fn send_request(&self, request_id: u32, request: http::Request) {
        info!("JavaPlatform.send_http_request NOT IMPLEMENTED");
        // if let Err(err) = self.send_http_request(request_id, request) {
       // error!("JavaPlatform.send_http_request failed: {:?}", err);
        // }
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

        Ok(connection)
    }

    fn on_start_call(
        &self,
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId, 
        direction: CallDirection,
        call_media_type: CallMediaType,
    ) -> Result<()> {
        info!(
            "on_start_call(): call_id: {}, direction: {}",
            call_id, direction
        );
        Ok(())
    }

    fn on_event(
        &self,
        remote_peer: &Self::AppRemotePeer,
        _call_id: CallId, 
        event: ApplicationEvent,
    ) -> Result<()> {
        info!("on_event(): {}", event); 
        Ok(())
    }

     fn on_network_route_changed(
        &self,
        remote_peer: &Self::AppRemotePeer,
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
        remote_peer: &Self::AppRemotePeer,
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
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        offer: signaling::Offer,
    ) -> Result<()> {
        info!("on_send_offer(): call_id: {}", call_id);
        Ok(())
    }

    fn on_send_answer(
        &self,
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        send: signaling::SendAnswer,
    ) -> Result<()> {
        info!(
            "on_send_answer(): call_id: {}",
            call_id
        );
        Ok(())
    }


    fn on_send_ice(
        &self,
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        send: signaling::SendIce,
    ) -> Result<()> {
        info!(
            "on_send_ice(): call_id: {}",
            call_id
        );
        Ok(())
    }

    fn on_send_hangup(
        &self,
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId,
        send: signaling::SendHangup,
    ) -> Result<()> {
        info!("on_send_hangup(): call_id: {}", call_id);
        Ok(())
    }

    fn on_send_busy(&self, remote_peer: &Self::AppRemotePeer, call_id: CallId) -> Result<()> {
        info!("on_send_busy(): call_id: {}", call_id);
        Ok(())
    }

    fn send_call_message(
        &self,
        recipient_uuid: Vec<u8>,
        message: Vec<u8>,
        urgency: group_call::SignalingMessageUrgency,
    ) -> Result<()> {
        info!("send_call_message():");
        Ok(())
    }

    fn send_call_message_to_group(
        &self,
        group_id: Vec<u8>,
        message: Vec<u8>,
        urgency: group_call::SignalingMessageUrgency,
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
        app_call_context: &Self::AppCallContext,
        incoming_media: &Self::AppIncomingMedia,
    ) -> Result<()> {
        info!("connect_incoming_media():");
        Ok(())
    }

    fn disconnect_incoming_media(&self, app_call_context: &Self::AppCallContext) -> Result<()> {
        info!("disconnect_incoming_media():");
        Ok(())
    }

    /// Notify the application that an offer is too old.
    fn on_offer_expired(
        &self,
        remote_peer: &Self::AppRemotePeer,
        call_id: CallId, 
        age: Duration,
    ) -> Result<()> {
        info!("NOT IMPLEMENTED");
        Ok(())
    }

    /// Notify the application that the call is completely concluded
    fn on_call_concluded(&self, remote_peer: &Self::AppRemotePeer, call_id: CallId) -> Result<()> {
        info!("NOT IMPLEMENTED");
        Ok(())
    }

    fn group_call_ring_update(
        &self,
        group_id: group_call::GroupId,
        ring_id: group_call::RingId,
        sender: UserId,
        update: group_call::RingUpdate,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn request_membership_proof(&self, client_id: group_call::ClientId) {
        info!("NOT IMPLEMENTED")
    }

    fn request_group_members(&self, client_id: group_call::ClientId) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_connection_state_changed(
        &self,
        client_id: group_call::ClientId,
        connection_state: group_call::ConnectionState,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_network_route_changed(
        &self,
        client_id: group_call::ClientId,
        network_route: NetworkRoute,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_join_state_changed(
        &self,
        client_id: group_call::ClientId,
        join_state: group_call::JoinState,
    ) {
        info!("NOT IMPLEMENTED")
    }
    fn handle_remote_devices_changed(
        &self,
        client_id: group_call::ClientId,
        remote_device_states: &[group_call::RemoteDeviceState],
        _reason: group_call::RemoteDevicesChangedReason,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_incoming_video_track(
        &self,
        client_id: group_call::ClientId,
        remote_demux_id: DemuxId,
        incoming_video_track: VideoTrack,
    ) {
        info!("NOT IMPLEMENTED")
    }

    fn handle_peek_changed(
        &self,
        client_id: group_call::ClientId,
        peek_info: &PeekInfo,
        joined_members: &HashSet<UserId>,
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

    fn handle_ended(&self, client_id: group_call::ClientId, reason: group_call::EndReason) {
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
    fn handle_peek_result(&self, request_id: u32, peek_result: PeekResult) {
        info!("JavaPlatform::NYIhandle_peek_result(): id: {}", request_id);
    }

}
