extern crate log;
use core::slice;
use std::time::Duration;
use log::info;

use crate::common::{CallDirection, CallId, CallMediaType, DeviceId, Result};
use crate::core::signaling;

use crate::core::bandwidth_mode::BandwidthMode;
use crate::core::connection::Connection;
use crate::core::util::{ptr_as_mut, ptr_as_box};
use crate::error::RingRtcError;
use crate::java::call_manager::JavaCallManager;
use crate::java::java_platform::{JavaPlatform,PeerId};
use crate::lite::http;
use crate::webrtc;
use crate::webrtc::peer_connection_observer::PeerConnectionObserver;
use crate::webrtc::peer_connection::{PeerConnection,RffiPeerConnection};

fn init_logging() {
    env_logger::builder()
        .filter(None, log::LevelFilter::Debug)
        .init();
    println!("LOGINIT done");
    info!("INFO logging enabled");
}

#[repr(C)]
pub struct MyKey {
  pub data: [u8; 8]
}

#[repr(C)]
pub struct Opaque {
  pub len: usize,
  pub data: [u8; 256],
  pub rawdata: *const u8
}

#[repr(C)]
pub struct IcePack {
    rows: [byte_array;25],
    length: usize
}

#[no_mangle]
pub unsafe extern "C" fn createJavaPlatform() -> *mut JavaPlatform {
    init_logging();
    let platform = JavaPlatform::new();
    let platform_box = Box::new(platform);
    Box::into_raw(platform_box)
}

#[no_mangle]
pub unsafe extern "C" fn createCallManager(pm: u64) -> i64 {
    info! ("Need to create Java call_manager, pm at {}\n", pm);
    let platform = ptr_as_box(pm as *mut JavaPlatform).unwrap() ;
    match create_java_call_manager(*platform) {
        Ok(v) => v as i64,
        Err(_e) => {
            println! ("Error creating Java CallManager");
            0
        }
    }
}

fn create_java_call_manager(platform: JavaPlatform) -> Result<*mut JavaCallManager> {
    // let platform = JavaPlatform::new();
    let http_client = http::DelegatingClient::new(platform.try_clone()?);
    let call_manager = JavaCallManager::new(platform, http_client)?;
    info!("Created cm, platform = {:?}", call_manager.platform());
    let call_manager_box = Box::new(call_manager);
    Ok(Box::into_raw(call_manager_box) )
}

#[no_mangle]
pub unsafe extern "C" fn set_first_callback(java_call_manager: u64, mycb: extern "C" fn(CallId, u64, CallDirection, CallMediaType)) {
    let call_manager = ptr_as_mut(java_call_manager as *mut JavaCallManager).unwrap() ;
    let mut java_platform = call_manager.platform().unwrap();
    // info!("Callback was to {:?}", java_platform.startCallback);
    java_platform.startCallback = mycb;
    info!("javaplatform = {:?}", java_platform);
    // info!("Callback stored to {:?}", mycb);
    info!("Current Thread = {:?}", std::thread::current().id());
}

#[no_mangle]
pub unsafe extern "C" fn set_create_connection_callback(java_call_manager: u64, mycb: extern "C" fn(u64, CallId) ->i64 ) {
    let call_manager = ptr_as_mut(java_call_manager as *mut JavaCallManager).unwrap() ;
    let mut java_platform = call_manager.platform().unwrap();
    java_platform.createConnectionCallback = mycb;
    info!("Created connection_callback javaplatform = {:?}", java_platform);
}

#[no_mangle]
pub unsafe extern "C" fn received_offer(
    call_manager: u64,
    call_id: CallId, 
    // remote_peer: <JavaPlatform as Platform>::AppRemotePeer,
    _remote_peer: u64,
    sender_device_id: DeviceId,
    opaque: Opaque,
    age_sec: u64,
    call_media_type: CallMediaType,
    receiver_device_id: DeviceId,
    receiver_device_is_primary: bool,
    sender_identity_key: MyKey,
    receiver_identity_key: MyKey,
) -> i64 {
    let call_manager = ptr_as_mut(call_manager as *mut JavaCallManager).unwrap() ;
    info! ("received offer for callid {} and callmanager {:?}", call_id, call_manager);
    println! ("opaquelen = {} and opaquedata = {:?}", opaque.len, opaque.data);
    println! ("sik0 = {}, sik = {:?}", sender_identity_key.data[0], sender_identity_key.data);
    let receiver_identity_key = receiver_identity_key.data.to_vec();
    let sender_identity_key = sender_identity_key.data.to_vec();
    let opvec = opaque.data.to_vec();
    let opvec2 = opvec[0..opaque.len].to_vec();
    let myremote_peer = PeerId::new();
    let received_offer = signaling::ReceivedOffer {
            // offer: signaling::Offer::new(call_media_type, opaque.data.get(0,opaque.len).to_vec()).unwrap(),
            offer: signaling::Offer::new(call_media_type, opvec2).unwrap(),
            age: Duration::from_secs(age_sec),
            sender_device_id,
            receiver_device_id,
            receiver_device_is_primary,
            sender_identity_key,
            receiver_identity_key,
        };
    let result = call_manager.received_offer(
             myremote_peer,
             call_id,
             received_offer);
    info!("RESULT of received_offer = {:?}", result);
    16
}

#[no_mangle]
pub unsafe extern "C" fn proceed(
    call_manager: u64, call_id: u64, bandwidth_mode: i32, audio_levels_interval_millis:i32) {
    info!("JavaRing, proceed called");
    let call_manager = ptr_as_mut(call_manager as *mut JavaCallManager).unwrap() ;
    let call_id = CallId::from(call_id);
    let bandwidth_mode = BandwidthMode::from_i32(bandwidth_mode);
    let context = 123.to_string();
    let audio_levels_interval = if audio_levels_interval_millis <= 0 { 
        None
    } else {
        Some(Duration::from_millis(audio_levels_interval_millis as u64))
    };
    call_manager.proceed(call_id, context, bandwidth_mode, audio_levels_interval);
}

#[no_mangle]
pub unsafe extern "C" fn createPeerConnection(
    peer_connection_factory: i64, native_connection_borrowed: i64) -> i64 {
    info!("createPeerConnection, fact = {} and conn = {}", peer_connection_factory, native_connection_borrowed);
    let my_native_connection = webrtc::ptr::Borrowed::from_ptr(
            native_connection_borrowed as *mut Connection<JavaPlatform>,
     );

    match create_peer_connection(peer_connection_factory, my_native_connection) {
        Ok(v) => v,
        Err(e) => {
            info!("ERROR creating peerConnection, return 0 to java layer");
            0
        }
    }
}

pub fn create_peer_connection(
    peer_connection_factory: i64, native_connection: webrtc::ptr::Borrowed<Connection<JavaPlatform>>
) -> Result<i64> {
    info!("JavaRing, createPeerConnection called, fact = {} ", peer_connection_factory);
    info!("JavaRing, conn = {:?}", native_connection);
        let connection = unsafe { native_connection.as_mut() }.ok_or_else(|| {
        RingRtcError::NullPointer(
            "create_peer_connection".to_owned(),
            "native_connection".to_owned(),
        )
    })?;
    info!("JavaRing, got conn = {:?}", connection);
    // native_connection is an un-boxed Connection<JavaPlatform> on the heap.
    // pass ownership of it to the PeerConnectionObserver.
    let pc_observer = PeerConnectionObserver::new(
        native_connection,
        false, /* enable_frame_encryption */
        false, /* enable_video_frame_event */
    )?;
    let rffi_pc = unsafe {
        webrtc::Arc::from_borrowed(webrtc::ptr::BorrowedRc::from_ptr(
            peer_connection_factory as *const RffiPeerConnection,
        ))
    };

    let peer_connection = PeerConnection::new(rffi_pc, None, None);

    connection.set_peer_connection(peer_connection)?;
    info!("connection: {:?}", connection);
    Ok(1)
}
#[no_mangle]
pub unsafe extern "C" fn received_ice(call_manager: u64, call_id: u64, sender_device_id: DeviceId, icepack: IcePack) {
    info!("JavaRing, received_ice with length = {}", icepack.length );
    let call_manager = ptr_as_mut(call_manager as *mut JavaCallManager).unwrap() ;
    let call_id = CallId::from(call_id);
    let mut ice_candidates = Vec::new();
    for j in 0..icepack.length {
        let row = &icepack.rows[j];
        let bytes = slice::from_raw_parts(row.bytes, row.length);
        let opaque = Vec::from(bytes);
        ice_candidates.push(signaling::IceCandidate::new(opaque));
    }
    call_manager.received_ice(
        call_id,
        signaling::ReceivedIce {
            ice: signaling::Ice {
                candidates: ice_candidates,
            },
            sender_device_id,
        },
    );
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct byte_array {
    bytes: *const u8,
    length: usize
}

#[repr(C)]
pub struct byte_array_2d {
    rows: [byte_array;10],
    length: usize
}
