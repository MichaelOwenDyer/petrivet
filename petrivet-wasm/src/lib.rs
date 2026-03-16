use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub fn hello() -> String {
    "petrivet-wasm loaded".to_string()
}