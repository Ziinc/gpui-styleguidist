# gpui-styleguidist

A living styleguide for [GPUI](https://www.gpui.rs) components, in the spirit of
[React Styleguidist](https://react-styleguidist.js.org). You write examples of your
components with **knobs** for their props, and you get:

- **A Styleguidist-style playground on the web.** `cargo styleguide build`
  compiles your styleguide to WebAssembly and writes a static site to host
  anywhere. It has a sidebar of components with a filter, and a page per
  component with its description, a props table and live examples. Each
  example has knobs and "View code", which shows the code for the current
  knob values, ready to copy. Links look like `#/Button` for a
  component, or `#!/button/primary?disabled=true` for one example in isolation.
- **The same styleguide natively.** `cargo run --example styleguide` opens it
  in a window.
- **Previews in rustdoc and on docs.rs.** `-- render` renders every example,
  theme and knob combination headlessly to PNGs under `styleguide/`.
  `#[preview("button/primary")]` embeds them in an item's docs. The generated
  `styleguide/header.html` turns each preview into a small playground: knobs
  swap snapshots and rewrite "View code" with a Copy button, and "Open live playground"
  embeds the hosted wasm example in an iframe. docs.rs serves rustdoc pages
  without a Content-Security-Policy, so all of this works there.
- **Visual regression tests.** `-- check` re-renders the snapshots in CI and
  fails on pixel changes.

It works with any GPUI component library. The demo in
[`examples/kit-demo`](examples/kit-demo) documents components built on
[gpui-kit](https://gpui-kit.com).

## Writing examples

```rust
use gpui_styleguidist::{Example, Styleguide};

pub fn styleguide() -> Styleguide {
    Styleguide::new("My components")
        .assets(my_assets())
        .setup(gpui_kit::init)
        // Host every window in the app's root view (gpui-kit needs its Root).
        .open_window(|options, frame, cx| Ok(gpui_kit::open_window(options, cx, frame)?.0))
        .themes(["light", "dark"], |theme, cx| {
            let mode = if theme == "dark" { ThemeMode::Dark } else { ThemeMode::Light };
            Theme::change(mode, None, cx);
        })
        .background(|cx| cx.theme().background)
        .describe("Button", "Buttons trigger actions.")
        .example(
            example!("button/primary", |knobs, _, _| {
                Button::new("save")
                    .label(knobs.text("label", "Save"))
                    .with_size(knobs.pick(
                        "size",
                        &[("small", Size::Small), ("medium", Size::Medium)],
                        "medium",
                    ))
                    .disabled(knobs.bool("disabled", false))
            })
            .component("Button")
            .description("Use one primary button per surface."),
        )
}
```

- **"View code" is code you can paste.** `example!` captures the closure
  body as written and turns every `knobs.*(…)` read into a slot. "View code"
  fills each slot with the knob's current value, so with size set to small
  and disabled on it shows `.with_size(Size::Small).disabled(true)`, and Copy
  puts that on the clipboard. Read knobs inline where the value is used, and
  use `pick` for values that aren't strings, numbers or bools: it shows the
  option's expression as written. `Example::new(…).source(…)` is the
  macro-free form, which shows fixed text.

- **Knobs are declared by reading them.** They include `bool`, `select`,
  `pick`, `text`, `text_samples` and `number`. The gallery shows a control for each
  one, and `render` snapshots every finite combination (bools, selects, text
  samples) up to `variant_limit`.
- **`example!(…)` is for stateless elements.** `example!(view …)` is for
  components that need an entity, such as an input's `InputState`. It is
  rebuilt on every knob change.
- **Knobs build the props table.** The knobs a component's examples read make
  up its "Props & methods" table.

The entry point is an ordinary example target:

```rust
// examples/styleguide.rs
fn main() {
    gpui_styleguidist::main(my_crate::styleguide());
}
```

```toml
# Cargo.toml
[dependencies]
gpui-styleguidist = { version = "0.1", default-features = false, features = ["macros"] }

[target.'cfg(not(target_family = "wasm"))'.dev-dependencies]
gpui-styleguidist = { version = "0.1", features = ["gallery", "render"] }

[target.'cfg(target_family = "wasm")'.dev-dependencies]
gpui-styleguidist = { version = "0.1", features = ["web"] }

[[example]]
name = "styleguide"
```

## Building the web styleguide

```sh
cargo install cargo-styleguide
cargo install wasm-bindgen-cli --version <the wasm-bindgen version in Cargo.lock>
rustup toolchain install nightly --target wasm32-unknown-unknown

cargo styleguide serve            # build (debug) and serve on http://127.0.0.1:8000
cargo styleguide build --release  # write target/styleguide/<package>/
# Smaller download: add a profile with opt-level = "z", lto = true and
# codegen-units = 1, then pass --profile <name> (see this repo's Cargo.toml).
```

The site is `index.html`, `styleguide.js` and `styleguide_bg.wasm`. Upload the
folder as it is.

On the web:

- **Nightly is required.** GPUI's wasm scheduler uses unstable `std::arch`
  APIs. `cargo styleguide` runs the wasm build through `rustup run nightly`;
  use `--toolchain` to pick another toolchain.
- **There are no system fonts.** The `web` feature bundles IBM Plex Sans for
  GPUI's `.SystemUIFont`. Add any other font your theme names with
  `Styleguide::fonts`.
- **Assets must work in the browser.** gpui-kit fetches icons over HTTP on the
  web (see the demo's `assets()`).

## Reloading while you work

```sh
cargo styleguide dev               # native: rebuild and reopen on every save
cargo styleguide serve --watch     # web: rebuild the wasm and reload the page
```

Both watch the package and every local crate it builds from.

- **`dev`** rebuilds the native styleguide and reopens it at the same example,
  theme and knobs. Natively, a rebuild usually takes a few seconds.
- **`serve --watch`** rebuilds the wasm and reloads the page. The URL keeps
  the example, theme and knobs. Wasm rebuilds are slower than native ones, so
  prefer `dev` for quick iteration.
- **A failed build keeps the last good one running.** The errors print in the
  terminal.
- **Knobs never reset on their own.** Every value you set is remembered per
  example, across reloads, rebuilds, theme switches and navigation, until you
  press "Reset knobs". The web keeps them in `localStorage`; natively they
  sit next to the styleguide executable in `<executable>.knobs.json`.

This rebuilds and restarts; it does not patch the running code.
[subsecond](https://crates.io/crates/subsecond) hot-patching only patches the
binary crate, and your components live in a library.

## Previews in rustdoc and on docs.rs

```rust
/// A button.
#[gpui_styleguidist::preview("button/primary")]
pub struct Button { /* … */ }

// In module or crate docs:
#![doc = gpui_styleguidist::preview_html!("button/primary", variants = false)]
```

```toml
[package.metadata.docs.rs]
rustdoc-args = ["--html-in-header", "styleguide/header.html"]
```

```sh
cargo run --example styleguide -- render   # write styleguide/ (commit it)
cargo run --example styleguide -- check    # compare; mismatches go to target/styleguide-check/
cargo rustdoc --open -- --html-in-header styleguide/header.html   # what docs.rs builds
```

- **Snapshots are committed and inlined.** docs.rs builds without a GPU or
  network, so the macros inline the committed PNGs as data URLs. Keep the
  snapshots in the published package; crates.io allows 10 MB.
- **Missing snapshots don't break the build.** A missing snapshot shows a note
  in the docs instead.
- **Previews can link to the hosted playground.** Call
  `Styleguide::playground_url(url)` before rendering, and each preview gets
  "Open live playground", which embeds `<url>#!/<id>?<knobs>`.

Rendering uses GPUI's real offscreen renderer: Metal on macOS, WGPU on Linux
(lavapipe works in CI). Text rasterizes differently across platforms, so
render and check on the same platform.

## Version pinning

gpui-styleguidist pins the exact `gpui-pre` snapshot that gpui-kit 0.7.3
uses (`=0.3.9`). Your crate must resolve the same GPUI.

## Layout

| Path | What it is |
|---|---|
| `crates/styleguidist` | `gpui-styleguidist`: examples, knobs, the styleguide UI, the web runtime, render, check and the rustdoc header |
| `crates/macros` | `example!`, `#[preview]`, `preview_html!` |
| `crates/manifest` | The snapshot manifest format shared by the runtime and the macros |
| `crates/cli` | `cargo-styleguide`: the wasm static-site builder and preview server |
| `examples/kit-demo` | gpui-kit components documented end to end, with committed snapshots |

## Releasing

Bump `version` in the workspace `Cargo.toml` (and the matching
`workspace.dependencies` versions), merge, then publish a GitHub release tagged
`v<version>`. The Publish workflow checks the tag against the version and
publishes `gpui-styleguidist-manifest`, `gpui-styleguidist-macros`,
`gpui-styleguidist` and `cargo-styleguide` in that order, using the
`CARGO_REGISTRY_TOKEN` repository secret. Versions already on crates.io are
skipped, so a failed run can be re-run.

## License

Apache-2.0. The bundled IBM Plex Sans and JetBrains Mono fonts are under the SIL Open Font License; see
[`crates/styleguidist/fonts/OFL.txt`](crates/styleguidist/fonts/OFL.txt).
