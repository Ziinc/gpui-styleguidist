//! Macros for gpui-styleguidist. Use them through `gpui_styleguidist`.
//!
//! - `example!` registers an example and captures its source, which the
//!   styleguide shows under "View code".
//! - `#[preview]` and `preview_html!` read `styleguide/manifest.json` and the
//!   PNG snapshots of the crate being compiled and inline them into rustdoc as
//!   data URLs, so docs need no network, GPU or hosted images. A missing
//!   snapshot becomes a visible note in the docs, never a build error, so
//!   docs.rs builds cannot break on it.

mod code;

use base64::Engine as _;
use gpui_styleguidist_manifest::{self as manifest, ExampleEntry, Manifest};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::path::{Path, PathBuf};
use syn::{
    Expr, LitBool, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// `example!("button/primary", |knobs, window, cx| { … })` builds an
/// [`Example`] whose "View code" shows the closure body as written, with every
/// `knobs.*(…)` read replaced by the knob's current value, ready to copy.
/// `example!(view "input/empty", |knobs, window, cx| { … })` builds a stateful
/// one with `Example::view`.
#[proc_macro]
pub fn example(input: TokenStream) -> TokenStream {
    let ExampleArgs { view, id, closure } = parse_macro_input!(input as ExampleArgs);
    let (source, knobs) = match &closure {
        Expr::Closure(closure) => {
            let knobs = closure.inputs.first().and_then(|input| match input {
                syn::Pat::Ident(pat) => Some(pat.ident.to_string()),
                syn::Pat::Type(pat) => match pat.pat.as_ref() {
                    syn::Pat::Ident(pat) => Some(pat.ident.to_string()),
                    _ => None,
                },
                _ => None,
            });
            let source = match closure.body.as_ref() {
                Expr::Block(block) => block
                    .block
                    .brace_token
                    .span
                    .join()
                    .source_text()
                    .map(|text| dedent(text.trim_start_matches('{').trim_end_matches('}'))),
                body => Some(quote!(#body).to_string()),
            };
            (source.unwrap_or_default(), knobs)
        }
        _ => (String::new(), None),
    };
    let parts = code::parts(&source, knobs.as_deref())
        .into_iter()
        .map(|part| match part {
            code::Part::Text(text) => quote!(::gpui_styleguidist::manifest::CodePart::text(#text)),
            code::Part::Knob { name, options } => {
                let options = options.iter().map(|(label, code)| quote!((#label, #code)));
                quote!(::gpui_styleguidist::manifest::CodePart::knob(#name, [#(#options),*]))
            }
        });
    let constructor = if view { quote!(view) } else { quote!(new) };
    quote! {
        ::gpui_styleguidist::Example::#constructor(#id, #closure)
            .code(::std::vec![#(#parts),*])
    }
    .into()
}

struct ExampleArgs {
    view: bool,
    id: Expr,
    closure: Expr,
}

impl Parse for ExampleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let view = if input.peek(syn::Ident) && input.fork().parse::<syn::Ident>()? == "view" {
            input.parse::<syn::Ident>()?;
            true
        } else {
            false
        };
        let id = input.parse()?;
        input.parse::<Token![,]>()?;
        let closure = input.parse()?;
        input.parse::<Option<Token![,]>>()?;
        Ok(Self { view, id, closure })
    }
}

/// Removes blank edge lines and the indentation every line shares.
fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    let (Some(start), Some(end)) = (start, end) else {
        return String::new();
    };
    let lines = &lines[start..=end];
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| line.get(indent..).unwrap_or("").trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Appends an example's preview to the documentation of the item it is on.
///
/// `#[preview("button/primary")]`, `#[preview("button/primary", theme = "dark")]`,
/// `#[preview("button/primary", variants = false)]`.
#[proc_macro_attribute]
pub fn preview(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as PreviewArgs);
    let doc = doc_expr(&args);
    // The item arrives with its other attributes, `///` docs included. Insert
    // the preview after them so it follows the item's own summary.
    let mut tokens = proc_macro2::TokenStream::from(item).into_iter().peekable();
    let mut attributes = proc_macro2::TokenStream::new();
    while let Some(proc_macro2::TokenTree::Punct(punct)) = tokens.peek() {
        if punct.as_char() != '#' {
            break;
        }
        attributes.extend(tokens.next());
        attributes.extend(tokens.next());
    }
    let rest: proc_macro2::TokenStream = tokens.collect();
    quote! {
        #attributes
        #[doc = ""]
        #[doc = #doc]
        #rest
    }
    .into()
}

/// The preview HTML as a string expression, for `#![doc = preview_html!("id")]`
/// in module or crate docs.
#[proc_macro]
pub fn preview_html(args: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as PreviewArgs);
    doc_expr(&args).into()
}

struct PreviewArgs {
    id: LitStr,
    theme: Option<LitStr>,
    variants: bool,
}

impl Parse for PreviewArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let id: LitStr = input.parse()?;
        let mut args = PreviewArgs {
            id,
            theme: None,
            variants: true,
        };
        while input.parse::<Option<Token![,]>>()?.is_some() {
            if input.is_empty() {
                break;
            }
            let name: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match name.to_string().as_str() {
                "theme" => args.theme = Some(input.parse()?),
                "variants" => args.variants = input.parse::<LitBool>()?.value,
                other => {
                    return Err(syn::Error::new(
                        name.span(),
                        format!("unknown option `{other}`; expected `theme` or `variants`"),
                    ));
                }
            }
        }
        Ok(args)
    }
}

/// `concat!(<html>, <stamp comment>)`: including the stamp makes rustdoc rebuild
/// when `render` rewrites the snapshots.
fn doc_expr(args: &PreviewArgs) -> proc_macro2::TokenStream {
    let dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join(manifest::DIR);
    let html = match html(&dir, args) {
        Ok(html) => html,
        Err(reason) => missing(&args.id.value(), &reason),
    };
    let html = LitStr::new(&html, Span::call_site());
    let stamp = dir.join(manifest::STAMP);
    if stamp.exists() {
        let stamp = LitStr::new(&stamp.to_string_lossy(), Span::call_site());
        quote!(concat!(#html, "\n<!-- gpui-styleguidist ", include_str!(#stamp), " -->"))
    } else {
        quote!(#html)
    }
}

fn html(dir: &Path, args: &PreviewArgs) -> Result<String, String> {
    let id = args.id.value();
    let path = dir.join(manifest::FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("`{}` does not exist", display(&path)))?;
    let manifest: Manifest =
        serde_json::from_str(&text).map_err(|error| format!("invalid manifest: {error}"))?;
    if manifest.version != manifest::VERSION {
        return Err(format!(
            "manifest version {} is not {}; re-render",
            manifest.version,
            manifest::VERSION
        ));
    }
    let example = manifest
        .example(&id)
        .ok_or_else(|| "no example with this id has been rendered".to_string())?;
    let theme = match &args.theme {
        Some(theme) => theme.value(),
        None => example.themes.first().cloned().unwrap_or_default(),
    };
    let default = example
        .default_variant(&theme)
        .ok_or_else(|| format!("no snapshot for theme `{theme}`"))?;

    let mut out = format!(
        "<div class=\"gpui-styleguide\" data-example=\"{}\">\n",
        escape(&id)
    );
    out.push_str(&format!(
        "<figure class=\"gsg-preview\">{}</figure>\n",
        image(dir, example, default, None)?
    ));
    let others: Vec<_> = example
        .variants
        .iter()
        .filter(|variant| !std::ptr::eq(*variant, default))
        .collect();
    if args.variants && !others.is_empty() {
        out.push_str(&format!(
            "<details class=\"gsg-variants\"><summary>Variants ({})</summary><div class=\"gsg-grid\">",
            others.len()
        ));
        for variant in others {
            let caption = match (variant.key.is_empty(), example.themes.len() > 1) {
                (true, _) => variant.theme.clone(),
                (false, true) => format!("{} · {}", variant.theme, changed(example, variant)),
                (false, false) => changed(example, variant),
            };
            out.push_str(&format!(
                "<figure>{}<figcaption>{}</figcaption></figure>",
                image(dir, example, variant, Some(&caption))?,
                escape(&caption)
            ));
        }
        out.push_str("</div></details>\n");
    }
    if !example.code.is_empty() {
        let code = manifest::render_code(&example.code, &example.knobs, |_| None);
        // Newlines inside an HTML block are fine; blank lines would end it.
        let code = escape(&code).replace("\n\n", "\n&#32;\n");
        out.push_str(&format!(
            "<details class=\"gsg-code\"><summary>View code</summary><pre><code>{code}</code></pre></details>\n"
        ));
    }
    let data = serde_json::json!({
        "id": example.id,
        "title": example.title,
        "theme": theme,
        "themes": if args.variants { example.themes.clone() } else { vec![theme.clone()] },
        "knobs": if args.variants { serde_json::to_value(&example.knobs).unwrap() } else { serde_json::json!([]) },
        "playground": manifest.playground_url,
        "code": if args.variants { serde_json::to_value(&example.code).unwrap() } else { serde_json::json!([]) },
    });
    out.push_str(&format!(
        "<script type=\"application/json\" class=\"gsg-data\">{}</script>\n",
        data.to_string().replace("</", "<\\/")
    ));
    out.push_str("</div>");
    Ok(out)
}

/// The knobs a variant changes from the default, `tone=danger`.
fn changed(example: &ExampleEntry, variant: &manifest::VariantEntry) -> String {
    let changed: Vec<String> = example
        .knobs
        .iter()
        .filter_map(|knob| {
            let value = variant.values.get(&knob.name)?;
            (*value != knob.default).then(|| format!("{}={}", knob.name, value.key()))
        })
        .collect();
    if changed.is_empty() {
        "default".into()
    } else {
        changed.join(", ")
    }
}

fn image(
    dir: &Path,
    example: &ExampleEntry,
    variant: &manifest::VariantEntry,
    caption: Option<&str>,
) -> Result<String, String> {
    let path = dir.join(&variant.file);
    let bytes = std::fs::read(&path).map_err(|_| format!("`{}` is missing", display(&path)))?;
    let alt = match caption {
        Some(caption) => format!("{} ({caption})", example.title),
        None => example.title.clone(),
    };
    Ok(format!(
        "<img src=\"data:image/png;base64,{}\" width=\"{}\" height=\"{}\" alt=\"{}\" data-theme=\"{}\" data-key=\"{}\">",
        base64::engine::general_purpose::STANDARD.encode(bytes),
        variant.width,
        variant.height,
        escape(&alt),
        escape(&variant.theme),
        escape(&variant.key),
    ))
}

fn missing(id: &str, reason: &str) -> String {
    format!(
        "<div class=\"gpui-styleguide\"><p class=\"gsg-missing\">Preview <code>{}</code> is not rendered: {}. Run <code>cargo run --example styleguide -- render</code> and commit <code>styleguide/</code>.</p></div>",
        escape(id),
        escape(reason)
    )
}

fn display(path: &Path) -> String {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
