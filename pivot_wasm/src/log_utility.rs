use serde::Serialize;
use serde_wasm_bindgen::to_value;

pub fn log_to_console<T: Serialize>(value: &T) {
    let js_value = to_value(value).expect("failed to convert value to JsValue");
    web_sys::console::log_1(&js_value);
}
