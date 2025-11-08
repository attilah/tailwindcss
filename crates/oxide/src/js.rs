#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen::prelude::*;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use console_error_panic_hook;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[wasm_bindgen(start)]
pub fn wasm_init() {
    console_error_panic_hook::set_once();
}

/// JS-facing wrapper for `(candidate, position)` tuples.
#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen(js_name = CandidateWithPosition)
)]
#[derive(Debug, Clone)]
pub struct JsCandidateWithPosition {
    candidate: String,
    position: usize,
}

impl From<(String, usize)> for JsCandidateWithPosition {
    fn from(value: (String, usize)) -> Self {
        JsCandidateWithPosition {
            candidate: value.0,
            position: value.1,
        }
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen(js_class = "CandidateWithPosition")
)]
impl JsCandidateWithPosition {
    #[cfg_attr(
        all(target_arch = "wasm32", target_os = "unknown"),
        wasm_bindgen(getter)
    )]
    pub fn candidate(&self) -> String {
        self.candidate.clone()
    }

    #[cfg_attr(
        all(target_arch = "wasm32", target_os = "unknown"),
        wasm_bindgen(getter)
    )]
    pub fn position(&self) -> usize {
        self.position
    }
}
