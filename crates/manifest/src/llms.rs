//! Markdown documentation for language models, generated from a manifest: an
//! [`llms.txt`](https://llmstxt.org) index, one page per component, and every
//! page joined into one file. Each page lists the component's props with the
//! builder method that sets them, and the code for each example and state.

use crate::{CodePart, ExampleEntry, KnobControl, KnobEntry, KnobValue, Manifest, render_code};
use std::collections::BTreeMap;

/// The index file's name.
pub const INDEX: &str = "llms.txt";

/// The file that holds every component page.
pub const FULL: &str = "llms-full.txt";

/// The directory, next to [`INDEX`], that holds one page per component.
pub const PAGES: &str = "llms";

/// A documented component: its description and examples.
pub struct Component<'a> {
    pub name: String,
    pub description: String,
    pub examples: Vec<&'a ExampleEntry>,
    /// The page's file name without `.md`, unique among the components.
    file: String,
}

impl Component<'_> {
    /// The page's path relative to [`INDEX`]: `llms/ActionButton.md`.
    pub fn path(&self) -> String {
        format!("{PAGES}/{}.md", self.file)
    }
}

/// Every component that has examples, in the order the styleguide lists them:
/// described components first, then the rest as their examples appear.
pub fn components(manifest: &Manifest) -> Vec<Component<'_>> {
    let mut components: Vec<Component> = manifest
        .components
        .iter()
        .map(|component| Component {
            name: component.name.clone(),
            description: component.description.clone(),
            examples: Vec::new(),
            file: String::new(),
        })
        .collect();
    for example in &manifest.examples {
        let name = example
            .component
            .clone()
            .unwrap_or_else(|| example.id.split('/').next().unwrap_or(&example.id).into());
        match components
            .iter_mut()
            .find(|component| component.name == name)
        {
            Some(component) => component.examples.push(example),
            None => components.push(Component {
                name,
                description: String::new(),
                examples: vec![example],
                file: String::new(),
            }),
        }
    }
    components.retain(|component| !component.examples.is_empty());
    // Names that slug alike, such as `A B` and `A-B`, get numbered files.
    let mut files: Vec<String> = Vec::new();
    for component in &mut components {
        let base = slug(&component.name);
        let mut file = base.clone();
        let mut number = 2;
        while files.iter().any(|taken| taken.eq_ignore_ascii_case(&file)) {
            file = format!("{base}-{number}");
            number += 1;
        }
        files.push(file.clone());
        component.file = file;
    }
    components
}

/// The `llms.txt` index: a summary and a link to each component's page.
pub fn index(manifest: &Manifest) -> String {
    let mut out = format!("# {}\n\n", title(manifest));
    out.push_str(
        "> GPUI component documentation: each page lists a component's props, the builder \
         method that sets each one, and copy-pasteable Rust code for every example and state.\n\n",
    );
    out.push_str("## Components\n\n");
    for component in components(manifest) {
        out.push_str(&format!("- [{}]({})", component.name, component.path()));
        if let Some(line) = component
            .description
            .lines()
            .find(|line| !line.trim().is_empty())
        {
            out.push_str(&format!(": {}", line.trim()));
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "\n## Optional\n\n- [Full documentation]({FULL}): every component page in one file\n"
    ));
    out
}

/// Every component page in one document.
pub fn full(manifest: &Manifest) -> String {
    let mut out = format!("# {}\n", title(manifest));
    for component in components(manifest) {
        out.push('\n');
        out.push_str(page_body(&component, 2).trim_end());
        out.push('\n');
    }
    out
}

/// One component's page.
pub fn page(component: &Component) -> String {
    page_body(component, 1)
}

fn page_body(component: &Component, level: usize) -> String {
    let h = |depth: usize| "#".repeat(level + depth);
    let mut out = format!("{} {}\n\n", h(0), component.name);
    if !component.description.trim().is_empty() {
        out.push_str(component.description.trim());
        out.push_str("\n\n");
    }

    let props = props(component);
    if !props.is_empty() {
        out.push_str(&format!("{} Props\n\n", h(1)));
        out.push_str("| Prop | Set with | Type | Default |\n| --- | --- | --- | --- |\n");
        for prop in props {
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                prop.knob.name,
                prop.method
                    .map_or_else(|| "argument".to_string(), |method| format!("`.{method}()`")),
                cell(&prop.type_name),
                cell(&format!("`{}`", prop.default)),
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!("{} Examples\n\n", h(1)));
    for example in &component.examples {
        out.push_str(&format!("{} {}\n\n", h(2), example.title));
        if !example.description.trim().is_empty() {
            out.push_str(example.description.trim());
            out.push_str("\n\n");
        }
        if example.code.is_empty() {
            continue;
        }
        out.push_str(&rust(&render_code(&example.code, &example.knobs, |_| None)));
        for state in &example.states {
            out.push_str(&format!("{} State: {}\n\n", h(3), state.name));
            let changed = changed(example, &state.values);
            if !changed.is_empty() {
                out.push_str(&format!("Knobs: {changed}.\n\n"));
            }
            out.push_str(&rust(&example.code_for(&state.values)));
        }
    }
    out
}

struct Prop<'a> {
    knob: &'a KnobEntry,
    /// The builder method whose argument the knob is, `tone` for
    /// `.tone(knob)`; `None` for a constructor argument.
    method: Option<String>,
    type_name: String,
    default: String,
}

/// Every knob the component's examples read, first read first.
fn props<'a>(component: &Component<'a>) -> Vec<Prop<'a>> {
    let mut props: Vec<Prop> = Vec::new();
    for example in &component.examples {
        for knob in &example.knobs {
            if props.iter().any(|prop| prop.knob.name == knob.name) {
                continue;
            }
            let code = slot_code(&example.code, &knob.name);
            let default = match (&knob.default, code) {
                (KnobValue::Text(label), Some(code)) if !code.is_empty() => {
                    code.get(label).cloned().unwrap_or_else(|| label.clone())
                }
                (value, _) => value.literal(),
            };
            props.push(Prop {
                knob,
                method: method_before(&example.code, &knob.name),
                type_name: type_name(knob, code),
                default,
            });
        }
    }
    props
}

/// The type of a knob's argument, with its values where there are few.
fn type_name(knob: &KnobEntry, code: Option<&BTreeMap<String, String>>) -> String {
    match (&knob.control, code) {
        (KnobControl::Select { options }, Some(code)) if !code.is_empty() => {
            let values: Vec<&str> = options
                .iter()
                .filter_map(|option| code.get(option).map(String::as_str))
                .collect();
            let path = values
                .first()
                .and_then(|value| value.rsplit_once("::"))
                .map(|(path, _)| path.to_string())
                .filter(|path| {
                    values
                        .iter()
                        .all(|value| value.starts_with(&format!("{path}::")))
                });
            match path {
                Some(path) => format!("`{path}`: {}", code_list(&values)),
                None => code_list(&values),
            }
        }
        (KnobControl::Select { options }, _) => {
            let values: Vec<String> = options.iter().map(|option| format!("{option:?}")).collect();
            format!("`&str`: {}", code_list(&values))
        }
        (KnobControl::Bool, _) => "`bool`".into(),
        (KnobControl::Text { .. }, _) => "string".into(),
        (KnobControl::Number { min, max, .. }, _) => format!("`f64` in `{min:?}..={max:?}`"),
    }
}

fn code_list(values: &[impl AsRef<str>]) -> String {
    values
        .iter()
        .map(|value| format!("`{}`", value.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn slot_code<'a>(parts: &'a [CodePart], name: &str) -> Option<&'a BTreeMap<String, String>> {
    parts.iter().find_map(|part| match part {
        CodePart::Knob { knob, code } if knob == name => Some(code),
        _ => None,
    })
}

/// `tone` when the text before the knob's first slot ends in `.tone(`.
fn method_before(parts: &[CodePart], name: &str) -> Option<String> {
    let slot = parts
        .iter()
        .position(|part| matches!(part, CodePart::Knob { knob, .. } if knob == name))?;
    let CodePart::Text { text } = parts.get(slot.checked_sub(1)?)? else {
        return None;
    };
    let before = text.trim_end().strip_suffix('(')?.trim_end();
    let method: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let rest = before[..before.len() - method.len()].trim_end();
    (!method.is_empty() && rest.ends_with('.')).then_some(method)
}

/// The knobs a state sets away from their defaults, `tone = danger`.
fn changed(example: &ExampleEntry, values: &BTreeMap<String, KnobValue>) -> String {
    example
        .knobs
        .iter()
        .filter_map(|knob| {
            let value = values.get(&knob.name)?;
            (*value != knob.default).then(|| format!("`{}` = `{}`", knob.name, value.key()))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn rust(code: &str) -> String {
    format!("```rust\n{}\n```\n\n", code.trim_end())
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn title(manifest: &Manifest) -> &str {
    if manifest.title.is_empty() {
        "Components"
    } else {
        &manifest.title
    }
}

/// A file name for a component: `Action Button` becomes `Action-Button`.
fn slug(name: &str) -> String {
    if name.trim().is_empty() {
        return "component".into();
    }
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_before_finds_the_builder_call() {
        let parts = vec![
            CodePart::text("Button::new(\"ok\", "),
            CodePart::knob("label", []),
            CodePart::text(")\n    .tone("),
            CodePart::knob("tone", [("danger", "Tone::Danger")]),
            CodePart::text(")"),
        ];
        assert_eq!(method_before(&parts, "label"), None);
        assert_eq!(method_before(&parts, "tone").as_deref(), Some("tone"));
    }

    #[test]
    fn page_files_are_unique() {
        let example = |id: &str, component: &str| ExampleEntry {
            id: id.into(),
            title: id.into(),
            component: Some(component.into()),
            description: String::new(),
            code: Vec::new(),
            knobs: Vec::new(),
            states: Vec::new(),
            themes: Vec::new(),
            variants: Vec::new(),
        };
        let manifest = Manifest {
            examples: vec![example("a", "A B"), example("b", "A-B"), example("c", "")],
            ..Manifest::default()
        };
        let paths: Vec<String> = components(&manifest).iter().map(Component::path).collect();
        assert_eq!(paths, ["llms/A-B.md", "llms/A-B-2.md", "llms/component.md"]);
    }
}
