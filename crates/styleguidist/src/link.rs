//! The styleguide's location as a URL hash: `#/Component`, or
//! `#!/example-id?theme=dark&knob=value` for one example in isolation.
//!
//! The web build keeps it in the page URL. Natively, when the
//! `STYLEGUIDE_LINK_FILE` environment variable names a file, the gallery writes
//! it there, so `cargo styleguide dev` can reopen the same place after a
//! rebuild with `gallery --link`.

use crate::gallery::Route;
use std::collections::BTreeMap;

/// The environment variable naming the file the native gallery writes its
/// link to.
#[cfg(not(target_family = "wasm"))]
pub const LINK_FILE_ENV: &str = "STYLEGUIDE_LINK_FILE";

/// Records where the gallery is: in the page URL on the web, without adding a
/// history entry; natively in the `STYLEGUIDE_LINK_FILE` file, if set.
pub(crate) fn publish(route: &Route, theme: Option<&str>) {
    let hash = format(route, theme);
    #[cfg(all(feature = "web", target_family = "wasm"))]
    crate::web::replace_hash(&hash);
    #[cfg(not(target_family = "wasm"))]
    if let Some(path) = std::env::var_os(LINK_FILE_ENV)
        && let Err(error) = std::fs::write(&path, &hash)
    {
        crate::guide::log_error(&format!("writing {}: {error}", path.to_string_lossy()));
    }
    #[cfg(not(any(not(target_family = "wasm"), feature = "web")))]
    let _ = hash;
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

/// The hash for `route` in `theme`; [`parse`] reads it back.
pub(crate) fn format(route: &Route, theme: Option<&str>) -> String {
    let mut hash = match route {
        Route::Component(name) => format!("#/{}", encode(name)),
        Route::Isolated { id, .. } => format!("#!/{}", encode(id)),
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
    hash
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_round_trip() {
        let route = Route::Isolated {
            id: "button/primary".into(),
            knobs: [("label".to_string(), "Save & close".to_string())].into(),
        };
        let hash = format(&route, Some("dark"));
        assert_eq!(
            hash,
            "#!/button/primary?theme=dark&label=Save%20%26%20close"
        );
        assert_eq!(parse(&hash), (Some(route), Some("dark".into())));
        assert_eq!(
            parse("#/Button"),
            (Some(Route::Component("Button".into())), None)
        );
    }
}
