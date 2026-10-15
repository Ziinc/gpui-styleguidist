//! Browser glue for the wasm gallery.

use std::{rc::Rc, sync::Arc};

/// A single-threaded browser application. Glyphs the bundled fonts lack, such
/// as emoji and CJK typed into an input, fall back to the browser's canvas text.
pub(crate) fn application() -> gpui::Application {
    use gpui_web::{CanvasFontFallback, WebBackendPreference, WebPlatform};

    gpui_platform::web_init();
    let platform = Rc::new(WebPlatform::new_with_backend_and_font_fallback(
        false,
        WebBackendPreference::Auto,
        CanvasFontFallback::EmojiAndCjk,
    ));
    let http_client = Arc::new(platform.fetch_http_client());
    gpui::Application::with_platform(platform).with_http_client(http_client)
}

/// The example id in the page URL: `…/#button/primary`.
pub(crate) fn linked_example() -> Option<String> {
    let hash = web_sys::window()?.location().hash().ok()?;
    let id = hash.trim_start_matches('#');
    let id = percent_decode(id);
    (!id.is_empty()).then_some(id)
}

/// Points the page URL at `id` without adding a history entry, so a reload or
/// a shared link opens the same example.
pub(crate) fn link_example(id: &str) {
    if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
        history
            .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&format!("#{id}")))
            .ok();
    }
}

/// Undoes the percent-encoding browsers apply to hashes, for ids with spaces.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
