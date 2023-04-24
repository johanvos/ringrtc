#![allow(unused_parens)]

use jni::objects::{GlobalRef, JMethodID, JObject, JStaticMethodID, JString, JValue};
use jni::signature::{Primitive, ReturnType};
use jni::sys::{jbyteArray, jint, jlong, JNI_VERSION_1_6};
use jni::{JNIEnv, JavaVM};

use std::collections::HashMap;
use std::convert::TryInto;

use std::mem::transmute;
use std::os::raw::c_void;
use std::slice;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use libc::size_t;

use crate::common::{CallId, CallMediaType, DeviceId, Result};
use crate::core::bandwidth_mode::BandwidthMode;
use crate::core::call_manager::CallManager;
use crate::core::group_call;
use crate::core::group_call::{GroupId, SignalingMessageUrgency};
use crate::core::signaling;
use crate::core::util::ptr_as_mut;

use crate::java::jtypes::{
    JArrayByte, JByteArray, JByteArray2D, JPString, TringDevice,
};

use crate::lite::http;
use crate::lite::sfu::{GroupMember, UserId};
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

const JAVA_UTIL_LIST_CLASS: &str = "java/util/List";
const JAVA_UTIL_ARRAY_LIST_CLASS: &str = "java/util/ArrayList";

static mut JAVA_UTIL_LIST_ADD: Option<JMethodID> = None;
static mut JAVA_UTIL_ARRAY_LIST_CTOR: Option<JMethodID> = None;

const JAVA_CALLBACK_CLASS: &str = "io/privacyresearch/tring/TringServiceImpl";
static mut JAVA_HTTP: Option<JMethodID> = None;
static mut JAVA_DEVICES_CHANGED: Option<JMethodID> = None;
static mut JAVA_PEEK_RESULT: Option<JMethodID> = None;
static mut JAVA_PEEK_CHANGED: Option<JMethodID> = None;

static mut target_object: Option<GlobalRef> = None;

static mut jvm_box: i64 = 0;

/// cbindgen:ignore
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn JNI_OnLoad(vm: JavaVM, _: *mut c_void) -> jint {
    info!("Loading RUST tringlib");
    println!("pLoading RUST tringlib");

    let mut env = vm.get_env().expect("Cannot get reference to the JNIEnv");
    let mut r = JNI_VERSION_1_6;

    init_cache(&mut env);

    let java_box = Box::new(vm);
    // let boxx: Result<*mut JavaVM> = Ok(Box::into_raw(java_box));

    jvm_box = Box::into_raw(java_box) as i64;
    r
}

unsafe fn init_cache(env: &mut JNIEnv) -> Result<()> {
    JAVA_HTTP = Some(env.get_method_id(JAVA_CALLBACK_CLASS, "makeHttpRequest","(Ljava/lang/String;BI[B[B)V")?);
    JAVA_DEVICES_CHANGED = Some(env.get_method_id(JAVA_CALLBACK_CLASS, "handleRemoteDevicesChanged","(Ljava/util/List;)V")?);
    JAVA_PEEK_RESULT = Some(env.get_method_id(JAVA_CALLBACK_CLASS, "handlePeekResponse","(Ljava/util/List;[BLjava/lang/String;JJ)V")?);
    JAVA_PEEK_CHANGED = Some(env.get_method_id(JAVA_CALLBACK_CLASS, "handlePeekChanged","(Ljava/util/List;[BLjava/lang/String;JJ)V")?);
    JAVA_UTIL_LIST_ADD = Some(env.get_method_id(
        JAVA_UTIL_LIST_CLASS,
        "add",
        "(Ljava/lang/Object;)Z",
    )?);    
    JAVA_UTIL_ARRAY_LIST_CTOR = Some(env.get_method_id(JAVA_UTIL_ARRAY_LIST_CLASS, "<init>", "()V")?);

    Ok(())
}


// ===== JNI METHODS

/// cbindgen:ignore
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_privacyresearch_tring_TringServiceImpl_initializeNative(
    mut env: JNIEnv,
    obj: JObject,
    endpoint: i64) {
    println!("Initialize native RUST layer, obj = {:?}", obj);
    target_object = env.new_global_ref(obj).ok();
}

/// cbindgen:ignore
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_privacyresearch_tring_TringServiceImpl_requestVideo(
    mut env: JNIEnv,
    obj: JObject,
    endpoint: i64,
    clientid: u32,
    demuxid: u32) {
    info!("request video");
    requestVideo(endpoint, clientid, demuxid);
}

/*
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn Java_io_privacyresearch_tring_TringServiceImpl_ringrtcReceivedHttpResponse(
    mut env: JNIEnv,
    obj: JObject,
    endpoint: jlong,
    request_id: jlong,
    status_code: jint,
    body: jbyteArray,
) {
    match do_received_http_response(
        &env,
        endpoint,
        request_id,
        status_code,
        body,
    ) {
        Ok(v) => v,
        Err(e) => {
            println!("Fatal error");
        }
    }
}

pub fn do_received_http_response(
    env: &JNIEnv,
    endpoint: jlong,
    request_id: jlong,
    status_code: jint,
    jbody: jbyteArray,
) -> Result<()> {


    println!("receivedHttpResponse!");
    let body = if jbody.is_null() {
        error!("Invalid body"); 
        return Ok(()); 
    } else {
        env.convert_byte_array(jbody)?
    };

    let response = http::Response {
        status: (status_code as u16).into(),
        body,
    };

    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.received_http_response(request_id as u32, Some(response));
    Ok(())  

}
*/

// === OUTGOING JNI

fn make_http_request(url: String, method: i8, reqid: i32, data: Vec<u8>, body: Vec<u8>) {
    unsafe {
        let javavm = ptr_as_mut(jvm_box as *mut JavaVM).unwrap();
        let mut env = javavm.attach_current_thread_as_daemon().unwrap();
        let jurl = env.new_string(&url).unwrap();
        let jheaders = env.byte_array_from_slice(&data).unwrap();
        let jbody = env.byte_array_from_slice(&body).unwrap();

        let args = [JValue::Object(&jurl).as_jni(),
                    JValue::Byte(method).as_jni(),
                    JValue::Int(reqid).as_jni(),
                    JValue::Object(&jheaders).as_jni(),
                    JValue::Object(&jbody).as_jni()];
        let original_object = target_object.as_ref().clone().unwrap().as_obj();
        info!("Let's make a real http request, orig = {:?}",original_object);
        env.call_method_unchecked(&original_object, JAVA_HTTP.unwrap(), ReturnType::Primitive(Primitive::Void),&args);
    }
}

fn handle_peek_response(body: Vec<u8>) {
    unsafe {
    }
}
// == END JNI

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
    pub genericCallback: unsafe extern "C" fn(i32, JArrayByte),
    sender: Sender<Event>,
    report: Arc<dyn Fn() + Send + Sync>,
}

fn string_to_bytes(v:String) -> Vec<u8> {
    let mut answer:Vec<u8> = Vec::new();
    let ul = v.len() as u32;
    answer.extend_from_slice(&ul.to_be_bytes());
    answer.extend_from_slice(v.as_bytes());
    answer
}

impl EventReporter {
    fn new(
        statusCallback: extern "C" fn(u64, u64, i32, i32),
        answerCallback: extern "C" fn(JArrayByte),
        offerCallback: extern "C" fn(JArrayByte),
        iceUpdateCallback: extern "C" fn(JArrayByte),
        genericCallback: extern "C" fn(i32, JArrayByte),
        sender: Sender<Event>,
        report: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            statusCallback,
            answerCallback,
            offerCallback,
            iceUpdateCallback,
            genericCallback,
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
            Event::SendHttpRequest {
                request_id,
                request:
                    http::Request {
                        method,
                        url,
                        headers,
                        body,
                    },
            } => {
                info!("Request id = {}", request_id);
                info!("Requestmethod = {:?}", method);
                info!("Requesturl = {:?}", url);
                info!("Requestheaders = {:?}", headers);
                info!("Requestbody = {:?}", body);

                let mut hdr:Vec<u8> = Vec::new();
                for (name, value) in headers.iter() {
info!("Need to add to header: {} == {}", name.to_string(), value.to_string());
                    hdr.extend(string_to_bytes(name.to_string()));
                    hdr.extend(string_to_bytes(value.to_string()));
                }

                let mut bodyb:Vec<u8> = Vec::new();
                let bl = body.as_ref().map_or(0, |v| v.len());
                bodyb.extend_from_slice(&bl.to_be_bytes());
                bodyb.extend(body.unwrap_or_default());

                make_http_request(url, method as i8, request_id as i32, hdr, bodyb);
            }
            Event::SendCallMessage{recipient_uuid, message, urgency} => {
                info!("SendCallMessage! recuuid = {:?}, msg = {:?}, urg = {:?}", recipient_uuid, message, urgency);
                let mut payload:Vec<u8> = Vec::new();
                payload.extend(recipient_uuid);
                let mlen: i32 = message.len().try_into().unwrap();
                payload.extend_from_slice(&mlen.to_be_bytes());
                payload.extend(message);
                info!("payloadlength = {}", payload.len());
                let data = JArrayByte::new(payload);
                unsafe {
                    (self.genericCallback)(5, data);
                }
            }
            Event::SendCallMessageToGroup{group_id, message, urgency} => {
                info!("SendCallMessageToGroup! gid = {:?}, msg = {:?}, urg = {:?}", group_id, message, urgency);
            }
            Event::GroupUpdate(GroupUpdate::RequestMembershipProof(client_id)) => {
                info!("RMP");
                let mut payload:Vec<u8> = Vec::new();
                let cid: i32 = client_id.try_into().unwrap();
                payload.extend_from_slice(&cid.to_be_bytes());
                let data = JArrayByte::new(payload);
                unsafe {
                    (self.genericCallback)(3, data);
                }
                info!("invoked RequestMemebershipProof");
            }
            Event::GroupUpdate(GroupUpdate::RequestGroupMembers(client_id)) => {
                info!("RGM");
                let mut payload:Vec<u8> = Vec::new();
                let cid: i32 = client_id.try_into().unwrap();
                payload.extend_from_slice(&cid.to_be_bytes());
                let data = JArrayByte::new(payload);
                unsafe {
                    (self.genericCallback)(4, data);
                }
                info!("invoked RequestGroupMembers");
            }
            Event::GroupUpdate(GroupUpdate::ConnectionStateChanged(
                client_id,
                connection_state,
            )) => {
                let mut payload:Vec<u8> = Vec::new();
                let cid: i32 = client_id.try_into().unwrap();
                payload.extend_from_slice(&cid.to_be_bytes());
                let csid: i32 = connection_state as i32;
                payload.extend_from_slice(&csid.to_be_bytes());
                let data = JArrayByte::new(payload);
                unsafe {
                    (self.genericCallback)(2, data);
                }
                info!("invoked CSTATEChanged");
            }
            Event::GroupUpdate(GroupUpdate::NetworkRouteChanged(client_id, network_route)) => {
                info!("NYI NetworkRouteChanged");
            }
            Event::GroupUpdate(GroupUpdate::JoinStateChanged(client_id, join_state)) => {
                info!("NYI JoinStatesChanged");
            }
            Event::GroupUpdate(GroupUpdate::RemoteDeviceStatesChanged(
                client_id,
                remote_device_states,
            )) => {
                info!("RemoteDeviceStatesChanged [being implemented]");
                unsafe{
                    let javavm = ptr_as_mut(jvm_box as *mut JavaVM).unwrap();
                    let mut env = javavm.attach_current_thread_as_daemon().unwrap();
                    let jni_devices = env
                            .new_object_unchecked(
                                JAVA_UTIL_ARRAY_LIST_CLASS,
                                JAVA_UTIL_ARRAY_LIST_CTOR.unwrap(),
                                &[],
                            )
                            .unwrap();
                    for (i, remote_device_state) in remote_device_states.iter().enumerate() {
                        // let mut payload:Vec<u8> = Vec::new();
                        // payload.extend_from_slice(&remote_device_state.demux_id.to_be_bytes());
                        let jni_demux_id = match env.byte_array_from_slice(&remote_device_state.demux_id.to_be_bytes()) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                continue;
                            }
                        };
                        env.call_method_unchecked(
                            &jni_devices,
                            JAVA_UTIL_LIST_ADD.unwrap(),
                            ReturnType::Primitive(Primitive::Boolean),
                            &[JValue::Object(&jni_demux_id).as_jni()],
                        )
                        .expect(&format!(
                            "Couldn't invoke method {} on class {}",
                            "add", JAVA_UTIL_LIST_CLASS
                        ));
                    }
                    let args = [JValue::Object(&jni_devices).as_jni()];
                    let original_object = target_object.as_ref().clone().unwrap().as_obj();
                    env.call_method_unchecked(&original_object, JAVA_DEVICES_CHANGED.unwrap(), ReturnType::Primitive(Primitive::Void),&args);
                }
            }
            Event::GroupUpdate(GroupUpdate::PeekChanged {
                client_id,
                peek_info,
            }) => {
                info!("PeekChanged");
                let joined_members = peek_info.unique_users();
info!("in rust, peekchanged, joined = {:?}", joined_members);
                unsafe{
                    let javavm = ptr_as_mut(jvm_box as *mut JavaVM).unwrap();
                    let mut env = javavm.attach_current_thread_as_daemon().unwrap();
                    let jni_joined_members = env
                            .new_object_unchecked(
                                JAVA_UTIL_ARRAY_LIST_CLASS,
                                JAVA_UTIL_ARRAY_LIST_CTOR.unwrap(),
                                &[],
                            )
                            .unwrap();

                    for joined_member in joined_members {
                        let jni_opaque_user_id = match env.byte_array_from_slice(joined_member) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                continue;
                            }
                        };
                        env.call_method_unchecked(
                            &jni_joined_members,
                            JAVA_UTIL_LIST_ADD.unwrap(),
                            ReturnType::Primitive(Primitive::Boolean),
                            &[JValue::Object(&jni_opaque_user_id).as_jni()],
                        )
                        .expect(&format!(
                            "Couldn't invoke method {} on class {}",
                            "add", JAVA_UTIL_LIST_CLASS
                        ));


                    };
                    let jni_creator = match peek_info.creator.as_ref() {
                        None => JObject::null(),
                        Some(creator) => match env.byte_array_from_slice(creator) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                return Ok(());
                            }
                        },
                    };
info!("in rust, peekchanged, creator = {:?}", jni_creator);
                    let jni_era_id = match peek_info.era_id.as_ref() {
                        None => JObject::null(),
                        Some(era_id) => match env.new_string(era_id) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                return Ok(());
                            }
                        },
                    };
                    let jni_max_devices =50 as jlong;
                    let jni_device_count = peek_info.device_count as jlong;
                            let original_object = target_object.as_ref().clone().unwrap().as_obj();
                            let args = [JValue::Object(&jni_joined_members).as_jni(),
                                        JValue::Object(&jni_creator).as_jni(),
                                        JValue::Object(&jni_era_id).as_jni(),
                                        JValue::Long(jni_max_devices).as_jni(),
                                        JValue::Long(jni_device_count).as_jni()];
                    env.call_method_unchecked(&original_object, JAVA_PEEK_CHANGED.unwrap(), ReturnType::Primitive(Primitive::Void),&args);
                }
            }
            Event::GroupUpdate(GroupUpdate::PeekResult {
                request_id,
                peek_result,
            }) => {
                let peek_info = peek_result.unwrap_or_default();
info!("peekresult, info: {:?}", peek_info);
                let joined_members = peek_info.unique_users();
info!("peekresult, JOINED: {:?}", joined_members);
                unsafe{
                    let javavm = ptr_as_mut(jvm_box as *mut JavaVM).unwrap();
                    let mut env = javavm.attach_current_thread_as_daemon().unwrap();
                    let jni_joined_members = env
                            .new_object_unchecked(
                                JAVA_UTIL_ARRAY_LIST_CLASS,
                                JAVA_UTIL_ARRAY_LIST_CTOR.unwrap(),
                                &[],
                            )
                            .unwrap();

                    for joined_member in joined_members {
                        let jni_opaque_user_id = match env.byte_array_from_slice(joined_member) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                continue;
                            }
                        };
println!("GOT JM: {:?}", jni_opaque_user_id);
                        env.call_method_unchecked(
                            &jni_joined_members,
                            JAVA_UTIL_LIST_ADD.unwrap(),
                            ReturnType::Primitive(Primitive::Boolean),
                            &[JValue::Object(&jni_opaque_user_id).as_jni()],
                        )
                        .expect(&format!(
                            "Couldn't invoke method {} on class {}",
                            "add", JAVA_UTIL_LIST_CLASS
                        ));


                    };
                    let jni_creator = match peek_info.creator.as_ref() {
                        None => JObject::null(),
                        Some(creator) => match env.byte_array_from_slice(creator) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                return Ok(());
                            }
                        },
                    };
                    let jni_era_id = match peek_info.era_id.as_ref() {
                        None => JObject::null(),
                        Some(era_id) => match env.new_string(era_id) {
                            Ok(v) => JObject::from(v),
                            Err(error) => {
                                error!("{:?}", error); 
                                return Ok(());
                            }
                        },
                    };
                    let jni_max_devices =50 as jlong;
                    let jni_device_count = peek_info.device_count as jlong;
                            let original_object = target_object.as_ref().clone().unwrap().as_obj();
                            let args = [JValue::Object(&jni_joined_members).as_jni(),
                                        JValue::Object(&jni_creator).as_jni(),
                                        JValue::Object(&jni_era_id).as_jni(),
                                        JValue::Long(jni_max_devices).as_jni(),
                                        JValue::Long(jni_device_count).as_jni()];
                    env.call_method_unchecked(&original_object, JAVA_PEEK_RESULT.unwrap(), ReturnType::Primitive(Primitive::Void),&args);
                }

            }
            Event::GroupUpdate(GroupUpdate::Ended(client_id, reason)) => {
                info!("NYI ENDED");
            }
            Event::GroupUpdate(GroupUpdate::Ring {
                group_id,
                ring_id,
                sender,
                update,
            }) => {
                info!("[JV] GroupUpdate, gid = {:?}, ringid = {:?}, sender = {:?}, update = {:?}", group_id, ring_id, sender, update);
                let mut payload:Vec<u8> = Vec::new();
                let glen: i32 = group_id.len().try_into().unwrap();
                payload.extend_from_slice(&glen.to_be_bytes());
                payload.extend(group_id);
                let rid: i64 = ring_id.into();
                payload.extend_from_slice(&rid.to_be_bytes());
                payload.extend(sender);
                payload.push(update as u8);
                let data = JArrayByte::new(payload);
                unsafe {
                    (self.genericCallback)(1, data);
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
        info!("Handle group update {:?}", update);
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
    // javavm: Option<JavaVM>,
}

impl CallEndpoint {
    fn new<'a>(
        use_new_audio_device_module: bool,
        statusCallback: extern "C" fn(u64, u64, i32, i32),
        answerCallback: extern "C" fn(JArrayByte),
        offerCallback: extern "C" fn(JArrayByte),
        iceUpdateCallback: extern "C" fn(JArrayByte),
        genericCallback: extern "C" fn(i32, JArrayByte),
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

        // let javavm = None;
        let event_reporter = EventReporter::new(
            statusCallback,
            answerCallback,
            offerCallback,
            iceUpdateCallback,
            genericCallback,
            events_sender,
            move || {
                info!("[JV] EVENT_REPORTER, NYI");
                if event_reported.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            },
            // javavm,
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
            // javavm,
        })
    }

/*
    fn java_env(&self) -> Result<JNIEnv> {
        match self.javavm.get_env() {
            Ok(v) => Ok(v),
            Err(_e) => Ok(self.javavm.attach_current_thread_as_daemon()?),
        }
    }
*/


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
pub unsafe extern "C" fn initRingRTC(ts: JPString) -> i64 {
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
    genericCallback: extern "C" fn(i32, JArrayByte),
    videoFrameCallback: extern "C" fn(*const u8, u32, u32, size_t),
) -> i64 {
    let call_endpoint = CallEndpoint::new(
        false,
        statusCallback,
        answerCallback,
        offerCallback,
        iceUpdateCallback,
        genericCallback,
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
pub unsafe extern "C" fn setSelfUuid(endpoint: i64, ts: JPString) -> i64 {
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
    peerId: JPString,
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
    let peer_id = JPString::from(peerId);
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
pub unsafe extern "C" fn receivedOpaqueMessage(
    endpoint: i64,
    sender_juuid: JByteArray,
    sender_device_id: DeviceId,
    local_device_id: DeviceId,
    opaque: JByteArray,
    message_age_sec: u64) -> i64 {
    info!("Create opaque message!");
    let message = opaque.to_vec_u8();
    let sender_uuid = sender_juuid.to_vec_u8();
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.received_call_message(sender_uuid, sender_device_id, local_device_id, message, Duration::from_secs(message_age_sec));
1
}

#[no_mangle]
pub unsafe extern "C" fn receivedAnswer(
    endpoint: i64,
    peerId: JPString,
    call_id: u64,
    sender_device_id: u32,
    sender_key: JByteArray,
    receiver_key: JByteArray,
    opaque: JByteArray,
) -> i64 {
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let peer_id = JPString::from(peerId);
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
    peer_id: JPString,
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
    ice_user: JPString,
    ice_pwd: JPString,
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
pub unsafe extern "C" fn fillLargeArray(endpoint: i64, mybuffer: *mut u8) -> i64 {
    let zero = *mybuffer.offset(0);
    let first = *mybuffer.offset(1);
    let second = *mybuffer.offset(12);
    info!("VAL 1 = {} and VAL2 = {}", first, second);
    *mybuffer.offset(12) = 13;
    1
}

#[no_mangle]
pub unsafe extern "C" fn fillRemoteVideoFrame(endpoint: i64, mybuffer: *mut u8, len: usize) -> i64 {
    info!("Have to retrieve remote video frame");
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let frame = endpoint.incoming_video_sink.pop(0);
    if let Some(frame) = frame {
        let frame = frame.apply_rotation();
        let width: u32 = frame.width();
        let height: u32 = frame.height();
        let myframe: &mut [u8] = slice::from_raw_parts_mut(mybuffer, len);
        frame.to_rgba(myframe);
        info!(
            "Frame0 = {}, w = {}, h = {}",
            myframe[0],
            frame.width(),
            frame.height()
        );
        let mut size: i64 = (frame.width() << 16).into();
        size = size + frame.height() as i64;
        size
    } else {
        0
    }
}
/// Convert a byte[] with 32-byte chunks in to a GroupMember struct vector. 
fn deserialize_to_group_member_info(
    mut serialized_group_members: Vec<u8>,
) -> Result<Vec<GroupMember>> {
    if serialized_group_members.len() % 81 != 0 {
        error!(
            "Serialized buffer is not a multiple of 81: {}",
            serialized_group_members.len()
        );
        return Err(anyhow::Error::msg("Error deserializing groupmember"));
    }

    let mut group_members = Vec::new();
    for chunk in serialized_group_members.chunks_exact_mut(81) {
        group_members.push(GroupMember {
            user_id: chunk[..16].into(),
            member_id: chunk[16..].into(),
        })
    }

    Ok(group_members)
}

// Group Calls


#[no_mangle]
pub unsafe extern "C" fn peekGroupCall(endpoint: i64,
    mp: JByteArray, gm: JByteArray
) -> i64 {
    info!("PeekGroupCall in rust");
    let membership_proof = mp.to_vec_u8();
    let ser_group_members = gm.to_vec_u8();
    let group_members = deserialize_to_group_member_info(ser_group_members).unwrap();
    let endpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("peekGroupCall will now invoke pgc on call_manager, not fully implemented");
    let sfu = String::from("https://sfu.voip.signal.org");
    endpoint.call_manager.peek_group_call(1, sfu, membership_proof, group_members);
    info!("PeekGroupCall in rust done");
    1
}

#[no_mangle]
pub unsafe extern "C" fn panamaReceivedHttpResponse(endpoint: i64,
    request_id: u32, 
    status_code: u32, 
    jbody:JByteArray) -> i64 {
    let body = jbody.to_vec_u8();
    let response = http::Response {
        status: (status_code as u16).into(),
        body,
    };

    info!("receivedHttpResponse, request_id = {}, status_code = {}", request_id, status_code);
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.received_http_response(request_id as u32, Some(response));
    1
}

#[no_mangle]
pub unsafe extern "C" fn createGroupCallClient(endpoint: i64,
            group_id: JByteArray,
            sf_url: JPString,
            hkdf_extra_info: JByteArray) -> i64 {
    info!("We Need to create groupcallclient");
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let audio_levels_interval = None;
    let peer_connection_factory = callendpoint.peer_connection_factory.clone();
    let outgoing_audio_track = callendpoint.outgoing_audio_track.clone();
    let outgoing_video_track = callendpoint.outgoing_video_track.clone();
    let incoming_video_sink = callendpoint.incoming_video_sink.clone();
    info!("Need to create groupcallclient, got all params");
        let result = callendpoint.call_manager.create_group_call_client(
            group_id.to_vec_u8(),
            sf_url.to_string(),
            hkdf_extra_info.to_vec_u8(),
            audio_levels_interval,
            Some(peer_connection_factory),
            outgoing_audio_track,
            outgoing_video_track,
            Some(incoming_video_sink),
        );

    let mut client_id = 0;
    if let Ok(v) = result {
        client_id = v;
    }
    info!("And return the client_id");
    client_id as i64
}

#[no_mangle]
pub unsafe extern "C" fn setOutgoingAudioMuted (endpoint: i64,
            client_id: u32, muted: bool) -> i64 {
    info!("need to set audio muted to {}", muted);
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.outgoing_audio_track.set_enabled(!muted);
    callendpoint.call_manager.set_outgoing_audio_muted(client_id, muted);
    info!("Done setting outgoingaudiomuted");
    1
}

#[no_mangle]
pub unsafe extern "C" fn setOutgoingVideoMuted (endpoint: i64,
            client_id: u32, muted: bool) -> i64 {
    info!("need to set video muted to {}", muted);
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.outgoing_video_track.set_enabled(!muted);
    callendpoint.call_manager.set_outgoing_video_muted(client_id, muted);
    info!("Done setting outgoingvideomuted");
    1
}

/*
#[no_mangle]
pub unsafe extern "C" fn setBandwidthMode (endpoint: i64,
            client_id: u32, mode: i32) -> i64 {
    info!("need to set bandwidth mode to {}", mode);
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.set_bandwidth_mode(client_id, BandwithMode::from_i32(mode));
    info!("Done setting bandwidthmode");
    1
}
*/

#[no_mangle]
pub unsafe extern "C" fn group_connect(endpoint: i64,
            client_id: u32) -> i64 {
    info!("need to connect!");
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    info!("ask callmanager to connect");
    callendpoint.call_manager.connect(client_id);
    info!("asked callmanager to connect");
    1
}

#[no_mangle]
pub unsafe extern "C" fn setMembershipProof(endpoint: i64,
            client_id: u32,
            token: JByteArray) -> i64 {
    info!("need to set_membershipProof");
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.set_membership_proof(client_id, token.to_vec_u8());
    1
}

#[no_mangle]
pub unsafe extern "C" fn setGroupMembers(endpoint: i64,
            client_id: u32,
            group_info: JByteArray) -> i64 {
    info!("need to set_membershipProof");
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let ser_group_members = group_info.to_vec_u8();
    let group_members = deserialize_to_group_member_info(ser_group_members).unwrap();
    callendpoint.call_manager.set_group_members(client_id, group_members);
    1
}

#[no_mangle]
pub unsafe extern "C" fn setBandwidthMode(endpoint: i64,
            client_id: u32,
            bandwidth_mode: BandwidthMode) -> i64 {
    info!("need to set_bandwidth_mode");
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.set_bandwidth_mode(client_id, bandwidth_mode);
    1
}

#[no_mangle]
pub unsafe extern "C" fn join(endpoint: i64,
            client_id: u32) -> i64 {
    info!("need to join");
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    callendpoint.call_manager.join(client_id);
    1
}

#[no_mangle]
pub unsafe extern "C" fn requestVideo(endpoint: i64,
            client_id: u32, demux_id: u32) -> i64 {
    info!("need to request video width demux_id = {}", demux_id);
    let callendpoint = ptr_as_mut(endpoint as *mut CallEndpoint).unwrap();
    let mut rendered_resolutions: Vec<group_call::VideoRequest> = Vec::new();
    let width = 320 as u16;
    let height = 200 as u16;
    let framerate = None;
            let rendered_resolution = group_call::VideoRequest {
            demux_id,
            width,
            height,
            framerate,
        };

        rendered_resolutions.push(rendered_resolution);

    callendpoint.call_manager.request_video(client_id, rendered_resolutions, 1);
    1
}
