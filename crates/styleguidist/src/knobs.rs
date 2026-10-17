use gpui::SharedString;
use gpui_styleguidist_manifest::{KnobControl, KnobEntry, KnobValue};
use std::{cell::RefCell, collections::BTreeMap, ops::RangeInclusive, rc::Rc};

/// The editable props of one example.
///
/// An example declares a knob by reading it: `knobs.bool("disabled", false)`
/// returns the current value and registers the knob the first time it is read.
/// The gallery shows a control for every registered knob, and `render`
/// pre-renders the finite ones (bools, selects, text samples) as snapshots.
///
/// `Knobs` is a cheap handle; clones share the same values, so a stateful view
/// can keep one and read it in `render`.
#[derive(Clone, Default)]
pub struct Knobs(Rc<RefCell<State>>);

#[derive(Default)]
struct State {
    specs: Vec<KnobEntry>,
    values: BTreeMap<String, KnobValue>,
    /// Values from a URL or an earlier run, typed once the example declares
    /// the knob.
    raw: BTreeMap<String, String>,
    /// Where the gallery remembers changes, and the example they belong to.
    #[cfg(feature = "gallery")]
    store: Option<(Rc<crate::store::KnobStore>, String)>,
}

impl Knobs {
    #[cfg_attr(not(feature = "render"), allow(dead_code))]
    pub(crate) fn with_values(values: BTreeMap<String, KnobValue>) -> Self {
        Self(Rc::new(RefCell::new(State {
            values,
            ..State::default()
        })))
    }

    /// Knobs that start from untyped values, such as a playground URL's query.
    /// A value that does not fit its knob is ignored.
    #[cfg_attr(not(feature = "gallery"), allow(dead_code))]
    pub(crate) fn with_raw(raw: BTreeMap<String, String>) -> Self {
        let knobs = Self::default();
        knobs.0.borrow_mut().raw = raw;
        knobs
    }

    /// Knobs whose changes `store` remembers for example `id`.
    #[cfg(feature = "gallery")]
    pub(crate) fn persisting(self, store: Rc<crate::store::KnobStore>, id: String) -> Self {
        self.0.borrow_mut().store = Some((store, id));
        self
    }

    /// A switch.
    pub fn bool(&self, name: &str, default: bool) -> bool {
        match self.read(name, KnobControl::Bool, KnobValue::Bool(default)) {
            KnobValue::Bool(value) => value,
            _ => default,
        }
    }

    /// One of `options`; `default` must be one of them.
    pub fn select<'a>(&self, name: &str, options: &[&'a str], default: &'a str) -> &'a str {
        debug_assert!(
            options.contains(&default),
            "knob `{name}`: default `{default}` is not one of its options"
        );
        debug_assert!(
            options
                .iter()
                .enumerate()
                .all(|(index, option)| !options[..index].contains(option)),
            "knob `{name}`: options {options:?} repeat a label"
        );
        let control = KnobControl::Select {
            options: options.iter().map(|option| option.to_string()).collect(),
        };
        match self.read(name, control, KnobValue::Text(default.into())) {
            KnobValue::Text(value) => options
                .iter()
                .copied()
                .find(|option| *option == value)
                .unwrap_or(default),
            _ => default,
        }
    }

    /// One of `options`, each a label and the value it stands for; `default`
    /// is a label. With [`example!`](crate::example), "View code" shows the
    /// picked value's expression as written, such as `Tone::Danger`.
    ///
    /// ```ignore
    /// .tone(knobs.pick("tone", &[("primary", Tone::Primary), ("danger", Tone::Danger)], "primary"))
    /// ```
    pub fn pick<T: Clone>(&self, name: &str, options: &[(&str, T)], default: &str) -> T {
        let labels: Vec<&str> = options.iter().map(|(label, _)| *label).collect();
        let label = self.select(name, &labels, default);
        options
            .iter()
            .find(|(option, _)| *option == label)
            .or(options.first())
            .map(|(_, value)| value.clone())
            .expect("knob `pick` needs at least one option")
    }

    /// Free text, editable in the gallery. Snapshots show only `default`; use
    /// [`Knobs::text_samples`] to render more.
    pub fn text(&self, name: &str, default: &str) -> SharedString {
        self.text_samples(name, &[default])
    }

    /// Free text whose `samples` are all rendered as snapshots. The first
    /// sample is the default.
    pub fn text_samples(&self, name: &str, samples: &[&str]) -> SharedString {
        let default = samples.first().copied().unwrap_or_default();
        let control = KnobControl::Text {
            samples: samples.iter().map(|sample| sample.to_string()).collect(),
        };
        match self.read(name, control, KnobValue::Text(default.into())) {
            KnobValue::Text(value) => value.into(),
            _ => SharedString::from(default.to_string()),
        }
    }

    /// A number in `range`, edited in `step` increments. Snapshots show only
    /// `default`.
    pub fn number(&self, name: &str, default: f64, range: RangeInclusive<f64>, step: f64) -> f64 {
        debug_assert!(
            range.start().is_finite() && range.end().is_finite() && range.start() <= range.end(),
            "knob `{name}`: range {range:?} must be finite and not empty"
        );
        debug_assert!(
            range.contains(&default),
            "knob `{name}`: default {default} is outside {range:?}"
        );
        let control = KnobControl::Number {
            min: *range.start(),
            max: *range.end(),
            step,
        };
        match self.read(name, control, KnobValue::Number(default)) {
            KnobValue::Number(value) => value.clamp(*range.start(), *range.end()),
            _ => default,
        }
    }

    fn read(&self, name: &str, control: KnobControl, default: KnobValue) -> KnobValue {
        let mut state = self.0.borrow_mut();
        if !state.values.contains_key(name)
            && let Some(raw) = state.raw.remove(name)
            && let Some(value) = parse(&control, &raw)
        {
            state.values.insert(name.into(), value);
        }
        if !state.specs.iter().any(|spec| spec.name == name) {
            state.specs.push(KnobEntry {
                name: name.into(),
                control,
                default: default.clone(),
            });
        }
        state.values.get(name).cloned().unwrap_or(default)
    }

    /// Every knob read so far, in the order the example first read them.
    pub fn specs(&self) -> Vec<KnobEntry> {
        self.0.borrow().specs.clone()
    }

    /// The current value of `name`, falling back to its default.
    pub fn value(&self, name: &str) -> Option<KnobValue> {
        let state = self.0.borrow();
        state.values.get(name).cloned().or_else(|| {
            state
                .specs
                .iter()
                .find(|spec| spec.name == name)
                .map(|spec| spec.default.clone())
        })
    }

    /// Overrides `name`; the next render reads `value`.
    pub fn set(&self, name: &str, value: KnobValue) {
        let mut state = self.0.borrow_mut();
        #[cfg(feature = "gallery")]
        if let Some((store, id)) = &state.store {
            store.set(id, name, value.key());
        }
        state.values.insert(name.into(), value);
    }

    /// Resets every knob to its default.
    pub fn reset(&self) {
        let mut state = self.0.borrow_mut();
        #[cfg(feature = "gallery")]
        if let Some((store, id)) = &state.store {
            store.clear(id);
        }
        state.values.clear();
        state.raw.clear();
    }
}

pub(crate) fn parse(control: &KnobControl, raw: &str) -> Option<KnobValue> {
    match control {
        KnobControl::Bool => raw.parse().ok().map(KnobValue::Bool),
        KnobControl::Select { options } => options
            .iter()
            .any(|option| option == raw)
            .then(|| KnobValue::Text(raw.into())),
        KnobControl::Text { .. } => Some(KnobValue::Text(raw.into())),
        // As the knob reads it: finite, within its range.
        KnobControl::Number { min, max, .. } => raw
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| KnobValue::Number(value.clamp(*min, *max))),
    }
}

/// Every pre-renderable combination of `specs`, default first.
///
/// When the full cartesian product exceeds `limit`, it falls back to the
/// default plus every single-knob change, so snapshots still cover each value.
#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) fn variants(specs: &[KnobEntry], limit: usize) -> Vec<BTreeMap<String, KnobValue>> {
    let domains: Vec<(&str, Vec<KnobValue>)> = specs
        .iter()
        .map(|spec| {
            let mut values = vec![spec.default.clone()];
            let others: Vec<KnobValue> = match &spec.control {
                KnobControl::Bool => vec![KnobValue::Bool(false), KnobValue::Bool(true)],
                KnobControl::Select { options } => {
                    options.iter().cloned().map(KnobValue::Text).collect()
                }
                KnobControl::Text { samples } => {
                    samples.iter().cloned().map(KnobValue::Text).collect()
                }
                KnobControl::Number { .. } => Vec::new(),
            };
            values.extend(others.into_iter().filter(|value| *value != spec.default));
            (spec.name.as_str(), values)
        })
        .collect();

    let defaults: BTreeMap<String, KnobValue> = specs
        .iter()
        .map(|spec| (spec.name.clone(), spec.default.clone()))
        .collect();

    let product: usize = domains.iter().map(|(_, values)| values.len()).product();
    if product <= limit {
        let mut combos = vec![defaults];
        for (name, values) in &domains {
            combos = combos
                .into_iter()
                .flat_map(|combo| {
                    values.iter().map(move |value| {
                        let mut next = combo.clone();
                        next.insert(name.to_string(), value.clone());
                        next
                    })
                })
                .collect();
        }
        combos
    } else {
        let mut combos = vec![defaults.clone()];
        for (name, values) in &domains {
            for value in values.iter().skip(1) {
                let mut next = defaults.clone();
                next.insert(name.to_string(), value.clone());
                combos.push(next);
            }
        }
        combos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_values_from_links_stay_finite_and_in_range() {
        let control = KnobControl::Number {
            min: 0.0,
            max: 10.0,
            step: 1.0,
        };
        assert_eq!(parse(&control, "NaN"), None);
        assert_eq!(parse(&control, "-inf"), None);
        assert_eq!(parse(&control, "1e308"), Some(KnobValue::Number(10.0)));
        assert_eq!(parse(&control, "4"), Some(KnobValue::Number(4.0)));
    }

    fn knobs() -> Knobs {
        let knobs = Knobs::default();
        knobs.bool("disabled", false);
        knobs.select("variant", &["primary", "ghost", "danger"], "primary");
        knobs.number("width", 120., 0.0..=400., 10.);
        knobs
    }

    #[test]
    fn reading_registers_once_and_returns_overrides() {
        let knobs = knobs();
        knobs.set("variant", KnobValue::Text("ghost".into()));
        assert_eq!(
            knobs.select("variant", &["primary", "ghost", "danger"], "primary"),
            "ghost"
        );
        assert_eq!(knobs.specs().len(), 3);
        assert_eq!(knobs.value("disabled"), Some(KnobValue::Bool(false)));
    }

    #[test]
    fn pick_returns_the_value_for_the_selected_label() {
        let knobs = Knobs::default();
        let options = [("small", 1), ("large", 2)];
        assert_eq!(knobs.pick("size", &options, "large"), 2);
        knobs.set("size", KnobValue::Text("small".into()));
        assert_eq!(knobs.pick("size", &options, "large"), 1);
    }

    #[test]
    fn raw_values_are_typed_by_their_knob() {
        let raw = [("disabled", "true"), ("variant", "nope"), ("width", "42")]
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let knobs = Knobs::with_raw(raw);
        assert!(knobs.bool("disabled", false));
        assert_eq!(
            knobs.select("variant", &["primary", "ghost"], "primary"),
            "primary"
        );
        assert_eq!(knobs.number("width", 120., 0.0..=400., 10.), 42.);
    }

    #[test]
    fn variants_cover_the_product_when_it_fits() {
        let combos = variants(&knobs().specs(), 32);
        assert_eq!(combos.len(), 6);
        assert_eq!(combos[0]["disabled"], KnobValue::Bool(false));
        assert_eq!(combos[0]["variant"], KnobValue::Text("primary".into()));
    }

    #[test]
    fn variants_fall_back_to_single_changes_over_the_limit() {
        let combos = variants(&knobs().specs(), 4);
        // default, disabled=true, variant=ghost, variant=danger
        assert_eq!(combos.len(), 4);
        assert!(combos[1..].iter().all(|combo| {
            let changed = (combo["disabled"] != KnobValue::Bool(false)) as u8
                + (combo["variant"] != KnobValue::Text("primary".into())) as u8;
            changed == 1
        }));
    }
}
