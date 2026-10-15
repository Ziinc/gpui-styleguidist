//! Browser glue for the wasm styleguide.

use crate::gallery::Route;
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

/// The route and theme in the page URL.
pub(crate) fn current_link() -> (Option<Route>, Option<String>) {
    let hash = web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default();
    crate::link::parse(&hash)
}

/// Replaces the page URL's hash without adding a history entry, so a reload
/// or a shared link opens the same place.
pub(crate) fn replace_hash(hash: &str) {
    if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
        history
            .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(hash))
            .ok();
    }
}
