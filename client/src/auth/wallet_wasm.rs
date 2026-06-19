#[cfg(target_arch = "wasm32")]
use sapp_jsutils::JsObject;

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn wallet_is_available() -> i32;
    fn wallet_sign_start(message: JsObject) -> i32;
    fn wallet_sign_poll(request_id: i32) -> i32;
    fn wallet_sign_result(request_id: i32) -> JsObject;
    fn wallet_sign_error(request_id: i32) -> JsObject;
    fn wallet_sign_cleanup(request_id: i32);
}

#[cfg(target_arch = "wasm32")]
pub fn is_wallet_available() -> bool {
    unsafe { wallet_is_available() == 1 }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_wallet_available() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
pub fn start_wallet_sign(message: &str) -> i32 {
    unsafe { wallet_sign_start(JsObject::string(message)) }
}

#[cfg(target_arch = "wasm32")]
pub fn poll_wallet_sign(request_id: i32) -> WalletSignPoll {
    let status = unsafe { wallet_sign_poll(request_id) };
    match status {
        0 => WalletSignPoll::Pending,
        1 => {
            let obj = unsafe { wallet_sign_result(request_id) };
            if obj.is_nil() {
                return WalletSignPoll::Failed("Empty wallet sign result".to_string());
            }
            let mut json = String::new();
            obj.to_string(&mut json);
            unsafe { wallet_sign_cleanup(request_id) };
            match serde_json::from_str::<WalletSignResult>(&json) {
                Ok(result) => WalletSignPoll::Done(result),
                Err(e) => WalletSignPoll::Failed(format!("Invalid wallet sign result: {e}")),
            }
        }
        _ => {
            let obj = unsafe { wallet_sign_error(request_id) };
            let mut err = String::from("Wallet sign failed");
            if !obj.is_nil() {
                obj.to_string(&mut err);
            }
            unsafe { wallet_sign_cleanup(request_id) };
            WalletSignPoll::Failed(err)
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WalletSignResult {
    pub pubkey: String,
    pub signature: String,
}

#[cfg(target_arch = "wasm32")]
pub enum WalletSignPoll {
    Pending,
    Done(WalletSignResult),
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
pub enum WalletSignPoll {
    Pending,
    Done(WalletSignResult),
    Failed(String),
}
