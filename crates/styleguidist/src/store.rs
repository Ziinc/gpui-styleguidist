//! Remembers every knob value the viewer sets, per example, so knobs survive
//! navigation, theme switches and reloads, and change only when the viewer
//! changes them or presses "Reset knobs".
//!
//! The web build keeps them in `localStorage`; natively they live in
//! `<executable>.knobs.json` next to the styleguide executable, which stays put
//! across rebuilds.

use crate::Knobs;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

type Values = BTreeMap<String, BTreeMap<String, String>>;

pub(crate) struct KnobStore {
    /// Example id to knob name to the value's key, as a URL would carry it.
    values: RefCell<Values>,
    location: Location,
}

impl gpui::Global for Shared {}

/// The store as a GPUI global.
pub(crate) struct Shared(pub Rc<KnobStore>);

impl KnobStore {
    /// The store for the styleguide named `name`, with what earlier runs saved.
    pub fn load(name: &str) -> Rc<Self> {
        let location = Location::new(name);
        let values = location
            .read()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Rc::new(Self {
            values: RefCell::new(values),
            location,
        })
    }

    /// Knobs for example `id`, starting from its saved values with
    /// `overrides`, such as a link's query, on top.
    pub fn knobs(self: &Rc<Self>, id: &str, overrides: &BTreeMap<String, String>) -> Knobs {
        if !overrides.is_empty() {
            let mut values = self.values.borrow_mut();
            let saved = values.entry(id.to_string()).or_default();
            saved.extend(overrides.clone());
        }
        let raw = self.values.borrow().get(id).cloned().unwrap_or_default();
        self.save();
        Knobs::with_raw(raw).persisting(self.clone(), id.to_string())
    }

    /// The values saved for example `id`.
    pub fn saved(&self, id: &str) -> BTreeMap<String, String> {
        self.values.borrow().get(id).cloned().unwrap_or_default()
    }

    pub fn set(&self, id: &str, name: &str, key: String) {
        self.values
            .borrow_mut()
            .entry(id.to_string())
            .or_default()
            .insert(name.to_string(), key);
        self.save();
    }

    pub fn clear(&self, id: &str) {
        self.values.borrow_mut().remove(id);
        self.save();
    }

    fn save(&self) {
        let values = self.values.borrow();
        if let Ok(text) = serde_json::to_string(&*values) {
            self.location.write(&text);
        }
    }
}

#[cfg(not(target_family = "wasm"))]
struct Location(Option<std::path::PathBuf>);

#[cfg(not(target_family = "wasm"))]
impl Location {
    fn new(_: &str) -> Self {
        Self(std::env::current_exe().ok().map(|exe| {
            let mut path = exe.into_os_string();
            path.push(".knobs.json");
            path.into()
        }))
    }

    fn read(&self) -> Option<String> {
        std::fs::read_to_string(self.0.as_ref()?).ok()
    }

    fn write(&self, text: &str) {
        if let Some(path) = &self.0
            && let Err(error) = std::fs::write(path, text)
        {
            crate::guide::log_error(&format!("saving knobs to {}: {error}", path.display()));
        }
    }
}

#[cfg(target_family = "wasm")]
struct Location(String);

#[cfg(target_family = "wasm")]
impl Location {
    fn new(name: &str) -> Self {
        Self(format!("gpui-styleguidist:knobs:{name}"))
    }

    #[cfg(feature = "web")]
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    #[cfg(feature = "web")]
    fn read(&self) -> Option<String> {
        Self::storage()?.get_item(&self.0).ok()?
    }

    #[cfg(feature = "web")]
    fn write(&self, text: &str) {
        if let Some(storage) = Self::storage() {
            // Fails only when storage is full or disabled; knobs then last
            // until the page reloads.
            storage.set_item(&self.0, text).ok();
        }
    }

    #[cfg(not(feature = "web"))]
    fn read(&self) -> Option<String> {
        None
    }

    #[cfg(not(feature = "web"))]
    fn write(&self, _: &str) {}
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use gpui_styleguidist_manifest::KnobValue;

    fn store(dir: &std::path::Path) -> Rc<KnobStore> {
        Rc::new(KnobStore {
            values: RefCell::default(),
            location: Location(Some(dir.join("knobs.json"))),
        })
    }

    #[test]
    fn knobs_survive_a_new_store_until_reset() {
        let dir = std::env::temp_dir().join(format!("gsg-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = store(&dir);
        let knobs = first.knobs("button", &BTreeMap::new());
        knobs.bool("disabled", false);
        knobs.set("disabled", KnobValue::Bool(true));

        // A reload: a new store reads what the first one saved.
        let reloaded = store(&dir);
        *reloaded.values.borrow_mut() =
            serde_json::from_str(&reloaded.location.read().unwrap()).unwrap();
        let knobs = reloaded.knobs("button", &BTreeMap::new());
        assert!(knobs.bool("disabled", false));

        // A link's values win over saved ones.
        let link = [("disabled".to_string(), "false".to_string())].into();
        assert!(!reloaded.knobs("button", &link).bool("disabled", false));

        knobs.reset();
        assert!(
            !reloaded
                .knobs("button", &BTreeMap::new())
                .bool("disabled", false)
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
