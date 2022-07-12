use crate::common::{Result};
use crate::java::call_manager::JavaCallManager;
use crate::java::java_platform::JavaPlatform;
use crate::lite::http;

#[no_mangle]
pub unsafe extern "C" fn createCallManager() -> i64 {
    print! ("Need to create Java call_manager\n");
    match create_java_call_manager() {
        Ok(v) => v,
        Err(e) => {
            println! ("Error creating Java CallManager");
            0
        }
    }
}

fn create_java_call_manager() -> Result<i64> {
    let platform = JavaPlatform::new();
    let http_client = http::DelegatingClient::new(platform.try_clone()?);
    let call_manager = JavaCallManager::new(platform, http_client)?;
    let call_manager_box = Box::new(call_manager);
    Ok(Box::into_raw(call_manager_box) as i64)
}

#[no_mangle]
pub unsafe extern "C" fn gotOffer() {
    print! ("Got offer from Java layer!\n");
}
