//! Splits an example's source into verbatim text and knob slots, so "View
//! code" can show `.disabled(true)` where the source reads
//! `.disabled(knobs.bool("disabled", false))`.
//!
//! This works on the source text rather than on tokens, so the code keeps the
//! author's formatting. It only needs to tell code from string literals and
//! comments, and to match brackets.

/// A piece of the example's code.
#[derive(Debug, PartialEq)]
pub enum Part {
    Text(String),
    /// A knob read. `options` maps each label of a `pick` knob to the
    /// expression it stands for.
    Knob {
        name: String,
        options: Vec<(String, String)>,
    },
}

/// The knob methods whose reads become slots.
const METHODS: [&str; 6] = ["bool", "select", "pick", "text", "text_samples", "number"];

pub fn parts(source: &str, knobs: Option<&str>) -> Vec<Part> {
    let Some(knobs) = knobs.filter(|knobs| *knobs != "_") else {
        return vec![Part::Text(source.to_string())];
    };
    let bytes = source.as_bytes();
    let mut parts = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if let Some(end) = skip_literal(source, i) {
            i = end;
            continue;
        }
        if let Some((knob, end)) = knob_read(source, i, knobs) {
            if text_start < i {
                parts.push(Part::Text(source[text_start..i].to_string()));
            }
            parts.push(knob);
            i = end;
            text_start = end;
            continue;
        }
        i += source[i..].chars().next().map_or(1, char::len_utf8);
    }
    if text_start < source.len() {
        parts.push(Part::Text(source[text_start..].to_string()));
    }
    parts
}

/// `knobs.method("name", …)` starting at `start`, as a slot, and the index
/// just past its closing parenthesis.
fn knob_read(source: &str, start: usize, knobs: &str) -> Option<(Part, usize)> {
    let rest = source[start..].strip_prefix(knobs)?;
    let before = source[..start].chars().next_back();
    if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
        return None;
    }
    let rest = rest.trim_start().strip_prefix('.')?.trim_start();
    let method = rest
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()?;
    if !METHODS.contains(&method) {
        return None;
    }
    let rest = rest[method.len()..].trim_start();
    if !rest.starts_with('(') {
        return None;
    }
    let open = source.len() - rest.len();
    let close = closing(source, open)?;
    let args = split_top(&source[open + 1..close]);
    let name = string_literal(args.first()?)?;
    let options = if method == "pick" {
        pick_options(args.get(1)?)?
    } else {
        Vec::new()
    };
    Some((Part::Knob { name, options }, close + 1))
}

/// `&[("primary", Tone::Primary), …]` as label and expression pairs.
fn pick_options(arg: &str) -> Option<Vec<(String, String)>> {
    let list = arg.trim().trim_start_matches('&').trim();
    let list = list.strip_prefix('[')?.strip_suffix(']')?;
    split_top(list)
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .map(|item| {
            let item = item.trim().strip_prefix('(')?.strip_suffix(')')?;
            let pair = split_top(item);
            let [label, code] = pair.as_slice() else {
                return None;
            };
            let code = code.split_whitespace().collect::<Vec<_>>().join(" ");
            Some((string_literal(label)?, code))
        })
        .collect()
}

fn string_literal(text: &str) -> Option<String> {
    syn::parse_str::<syn::LitStr>(text.trim())
        .ok()
        .map(|lit| lit.value())
}

/// The index of the bracket closing the one at `open`.
fn closing(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        if let Some(end) = skip_literal(source, i) {
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Splits on commas outside brackets and literals; a trailing comma adds no
/// item.
fn split_top(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut items = Vec::new();
    let mut depth = 0isize;
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if let Some(end) = skip_literal(text, i) {
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                items.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if !text[start..].trim().is_empty() {
        items.push(&text[start..]);
    }
    items
}

/// When a string, raw string, char literal or comment starts at `i`, the index
/// just past it.
fn skip_literal(text: &str, i: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let rest = &text[i..];
    if rest.starts_with("//") {
        return Some(rest.find('\n').map_or(text.len(), |end| i + end));
    }
    if rest.starts_with("/*") {
        return Some(rest.find("*/").map_or(text.len(), |end| i + end + 2));
    }
    let ident_before = i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
    if !ident_before && (rest.starts_with("r\"") || rest.starts_with("r#")) {
        let hashes = rest[1..].bytes().take_while(|b| *b == b'#').count();
        if rest[1 + hashes..].starts_with('"') {
            let close = format!("\"{}", "#".repeat(hashes));
            let body = 2 + hashes;
            return Some(
                rest[body..]
                    .find(&close)
                    .map_or(text.len(), |end| i + body + end + close.len()),
            );
        }
    }
    match bytes[i] {
        b'"' => {
            let mut j = i + 1;
            while j < bytes.len() {
                match bytes[j] {
                    b'\\' => j += 2,
                    b'"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(text.len())
        }
        // A char literal, not a lifetime: `'x'`, `'\n'`, `'\u{2026}'`.
        b'\'' => {
            let mut chars = rest[1..].char_indices();
            match chars.next()? {
                (_, '\\') => rest[2..].find('\'').map(|end| i + 2 + end + 1),
                (_, c) => {
                    let after = 1 + c.len_utf8();
                    rest[after..].starts_with('\'').then_some(i + after + 1)
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(parts: &[Part]) -> String {
        parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => text.clone(),
                Part::Knob { name, options } if options.is_empty() => format!("<{name}>"),
                Part::Knob { name, options } => format!(
                    "<{name}:{}>",
                    options
                        .iter()
                        .map(|(label, code)| format!("{label}={code}"))
                        .collect::<Vec<_>>()
                        .join("|")
                ),
            })
            .collect()
    }

    #[test]
    fn knob_reads_become_slots() {
        let source = r#"Button::new("ok", knobs.text("label", "Save, (now)"))
    .tone(knobs.pick(
        "tone",
        &[("primary", Tone::Primary), ("danger", Tone::Danger)],
        "primary",
    ))
    .disabled(knobs.bool("disabled", false)) // knobs.bool("no", true)"#;
        assert_eq!(
            render(&parts(source, Some("knobs"))),
            r#"Button::new("ok", <label>)
    .tone(<tone:primary=Tone::Primary|danger=Tone::Danger>)
    .disabled(<disabled>) // knobs.bool("no", true)"#
        );
    }

    #[test]
    fn other_text_is_left_alone() {
        let source =
            r#"let s = "knobs.bool(\"x\", true)"; my_knobs.bool("y", true); knobs.clone()"#;
        assert_eq!(render(&parts(source, Some("knobs"))), source);
        assert_eq!(render(&parts(source, Some("_"))), source);
    }
}
