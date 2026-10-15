//! The snapshot manifest `styleguide render` writes, `styleguide check`
//! compares against and `#[preview]` embeds in rustdoc. It lives at
//! `styleguide/manifest.json` next to the PNG snapshots, and is plain data so
//! the doc macros can read it without compiling GPUI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The manifest format version this crate reads and writes.
pub const VERSION: u32 = 1;

/// Directory, relative to the crate's manifest directory, that holds
/// the manifest and snapshots.
pub const DIR: &str = "styleguide";

/// File name of the manifest inside [`DIR`].
pub const FILE: &str = "manifest.json";

/// A hash of the manifest, rewritten on every render. `#[preview]` includes it
/// so rustdoc rebuilds when snapshots change.
pub const STAMP: &str = "stamp";

/// The rustdoc `--html-in-header` file that makes previews interactive.
pub const HEADER: &str = "header.html";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub version: u32,
    /// The hosted wasm styleguide; docs open `<url>#!/<example id>` in an
    /// iframe as the live playground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playground_url: Option<String>,
    pub examples: Vec<ExampleEntry>,
}

impl Manifest {
    pub fn example(&self, id: &str) -> Option<&ExampleEntry> {
        self.examples.iter().find(|example| example.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExampleEntry {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The example's source, captured by `example!`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knobs: Vec<KnobEntry>,
    pub themes: Vec<String>,
    pub variants: Vec<VariantEntry>,
}

impl ExampleEntry {
    /// The variant rendered with every knob at its default, in `theme`.
    pub fn default_variant(&self, theme: &str) -> Option<&VariantEntry> {
        self.variants
            .iter()
            .find(|variant| variant.theme == theme && variant.is_default)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnobEntry {
    pub name: String,
    #[serde(flatten)]
    pub control: KnobControl,
    pub default: KnobValue,
}

/// How a knob is edited, and which values snapshots exist for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KnobControl {
    Bool,
    Select {
        options: Vec<String>,
    },
    /// Free text. Only `samples` are pre-rendered; other values need the live
    /// gallery.
    Text {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        samples: Vec<String>,
    },
    /// Only the default is pre-rendered; other values need the live gallery.
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum KnobValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

impl KnobValue {
    /// The value as it appears in a variant key: `true`, `primary`, `1.5`.
    pub fn key(&self) -> String {
        match self {
            KnobValue::Bool(value) => value.to_string(),
            KnobValue::Number(value) => value.to_string(),
            KnobValue::Text(value) => value.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariantEntry {
    pub theme: String,
    /// Canonical `name=value,…` key in knob declaration order; empty when the
    /// example has no knobs.
    pub key: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, KnobValue>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_default: bool,
    /// Path relative to [`DIR`].
    pub file: String,
    /// Logical (1x) size; the PNG itself is rendered at `scale`.
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    /// FNV-1a hash of the RGBA pixels, so pixel changes show in diffs.
    pub hash: String,
}

/// Builds the canonical variant key from knob values in declaration order.
pub fn variant_key<'a>(values: impl IntoIterator<Item = (&'a str, &'a KnobValue)>) -> String {
    values
        .into_iter()
        .map(|(name, value)| format!("{name}={}", value.key()))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_entries_round_trip_through_json() {
        let knob = KnobEntry {
            name: "variant".into(),
            control: KnobControl::Select {
                options: vec!["primary".into(), "ghost".into()],
            },
            default: KnobValue::Text("primary".into()),
        };
        let json = serde_json::to_string(&knob).unwrap();
        assert_eq!(
            json,
            r#"{"name":"variant","type":"select","options":["primary","ghost"],"default":"primary"}"#
        );
        assert_eq!(serde_json::from_str::<KnobEntry>(&json).unwrap(), knob);
    }

    #[test]
    fn variant_keys_follow_declaration_order() {
        let disabled = KnobValue::Bool(true);
        let variant = KnobValue::Text("ghost".into());
        assert_eq!(
            variant_key([("variant", &variant), ("disabled", &disabled)]),
            "variant=ghost,disabled=true"
        );
    }
}
