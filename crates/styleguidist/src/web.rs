//! Browser glue for the wasm styleguide.

use crate::gallery::Route;
use std::{collections::BTreeMap, rc::Rc, sync::Arc};

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
    parse(&hash)
}

/// `#/Component`, `#!/example-id?theme=dark&knob=value`, or a bare
/// `#example-id`, which opens the example's component.
pub(crate) fn parse(hash: &str) -> (Option<Route>, Option<String>) {
    let hash = hash.trim_start_matches('#');
    let (path, query) = hash.split_once('?').unwrap_or((hash, ""));
    let mut knobs = BTreeMap::new();
    let mut theme = None;
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let (key, value) = (decode(key), decode(value));
        if key == "theme" {
            theme = Some(value);
        } else {
            knobs.insert(key, value);
        }
    }
    let route = if let Some(id) = path.strip_prefix("!/") {
        Some(Route::Isolated {
            id: decode(id).into(),
            knobs,
        })
    } else {
        let name = decode(path.trim_start_matches('/'));
        (!name.is_empty()).then(|| Route::Component(name.into()))
    };
    (route, theme)
}

/// Points the page URL at `route` without adding a history entry, so a reload
/// or a shared link opens the same place.
pub(crate) fn link(route: &Route, theme: Option<&str>) {
    let mut hash = match route {
        Route::Component(name) => format!("#/{}", encode(name)),
        Route::Isolated { id, .. } => format!("#!/{id}"),
    };
    let mut query: Vec<String> = Vec::new();
    if let Some(theme) = theme {
        query.push(format!("theme={}", encode(theme)));
    }
    if let Route::Isolated { knobs, .. } = route {
        query.extend(
            knobs
                .iter()
                .map(|(key, value)| format!("{}={}", encode(key), encode(value))),
        );
    }
    if !query.is_empty() {
        hash = format!("{hash}?{}", query.join("&"));
    }
    if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
        history
            .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&hash))
            .ok();
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Undoes percent-encoding and `+` for spaces.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = text
                .get(i + 1..i + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
