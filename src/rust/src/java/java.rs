extern crate log;
use std::time::Duration;
use log::info;


use crate::common::{CallId, CallMediaType, DeviceId, Result};
use crate::core::signaling;

use crate::core::util::ptr_as_mut;
use crate::java::call_manager::JavaCallManager;
use crate::java::java_platform::{JavaPlatform,PeerId};
use crate::lite::http;

fn init_logging() {
    env_logger::builder()
        .filter(None, log::LevelFilter::Debug)
        .init();
    println!("LOGINIT done");
    info!("INFO logging enabled");
}


#[no_mangle]
pub unsafe extern "C" fn createCallManager() -> i64 {
    init_logging();
    info! ("Need to create Java call_manager\n");
    match create_java_call_manager() {
        Ok(v) => v,
        Err(_e) => {
            println! ("Error creating Java CallManager");
            0
        }
    }
}

#[repr(C)]
pub struct MyKey {
  pub data: [u8; 8]
}

#[repr(C)]
pub struct Opaque {
  pub len: usize,
  pub data: [u8; 256]
}

#[no_mangle]
// pub unsafe extern "C" fn offerFromAppReceived(_opaque: Vec<u8>) {
pub unsafe extern "C" fn offerFromAppReceived() {
    println! ("Got offer with some bytes!")
}

#[no_mangle]
pub unsafe extern "C" fn dribbeldedribbel() {
    println! ("Got offer with some bytes!")
}

fn create_java_call_manager() -> Result<i64> {
    let platform = JavaPlatform::new();
    let http_client = http::DelegatingClient::new(platform.try_clone()?);
    let call_manager = JavaCallManager::new(platform, http_client)?;
    let call_manager_box = Box::new(call_manager);
    Ok(Box::into_raw(call_manager_box) as i64)
}

#[no_mangle]
pub unsafe extern "C" fn set_first_callback(java_call_manager: u64, mycb: extern "C" fn(i32)) {
    let call_manager = ptr_as_mut(java_call_manager as *mut JavaCallManager).unwrap() ;
    let mut java_platform = call_manager.platform().unwrap();
    java_platform.startCallback = mycb;
    info!("javaplatform = {:?}", java_platform);
    info!("Callback stored to {:?}", mycb);
    info!("Current Thread = {:?}", std::thread::current().id());

    mycb(3);
    info!("Callback invoked!");
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
    println! ("Sort of received offer for callid {} and callmanager ", call_id);
    let call_manager = ptr_as_mut(call_manager as *mut JavaCallManager).unwrap() ;
    println! ("WE ALSO GOT A CALLMANAGER NOW!");
    println! ("opaquelen = {} and opaquedata = {:?}", opaque.len, opaque.data);
    // let sender_identity_key = sender_identity_key.data.to_vec();
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
    println!("RESULT of received_offer = {:?}", result);
    16
}

#[no_mangle]
pub unsafe extern "C" fn gotMyOffer() {
    print! ("Got offer from Java layer!\n");
}
