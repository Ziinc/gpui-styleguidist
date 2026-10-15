# gpui-styleguidist

A living styleguide for [GPUI](https://www.gpui.rs) components, in the spirit of
[React Styleguidist](https://react-styleguidist.js.org). You write examples of your
components with **knobs** for their props, and you get:

- **A native gallery.** `cargo run --example styleguide` opens a window listing
  every example, with live knobs and theme switching.
- **The same gallery on the web.** `cargo styleguide build` compiles it to
  WebAssembly and writes a static site you can host anywhere (GitHub Pages,
  Netlify, your docs site). Every example has a deep link such as
  `#action-button/tones`.
- **Visual regression snapshots.** `-- render` renders every example, theme and
  knob combination headlessly to PNGs under `styleguide/`. `-- check` re-renders
  them in CI and fails on pixel changes.

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
        .example(
            Example::new("button/primary", |knobs, _, _| {
                Button::new("save")
                    .label(knobs.text("label", "Save"))
                    .disabled(knobs.bool("disabled", false))
            })
            .component("Button")
            .description("Use one primary button per surface."),
        )
}
```

- **Knobs are declared by reading them.** They include `bool`, `select`,
  `text`, `text_samples` and `number`. The gallery shows a control for each
  one, and `render` snapshots every finite combination (bools, selects, text
  samples) up to `variant_limit`.
- **`Example::new` is for stateless elements.** `Example::view` is for
  components that need an entity, such as an input's `InputState`. It is
  rebuilt on every knob change.

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
gpui-styleguidist = "0.1"

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

## Snapshots

```sh
cargo run --example styleguide -- render   # write styleguide/ (commit it)
cargo run --example styleguide -- check    # compare; mismatches go to target/styleguide-check/
```

Rendering uses GPUI's real offscreen renderer: Metal on macOS, WGPU on Linux
(lavapipe works in CI). Text rasterizes differently across platforms, so
render and check on the same platform.

## Version pinning

gpui-styleguidist pins the exact `gpui-pre` snapshot that gpui-kit 0.7.3
uses (`=0.3.9`). Your crate must resolve the same GPUI.

## Layout

| Path | What it is |
|---|---|
| `crates/styleguidist` | `gpui-styleguidist`: examples, knobs, the gallery, the web runtime, render and check |
| `crates/cli` | `cargo-styleguide`: the wasm static-site builder and preview server |
| `examples/kit-demo` | gpui-kit components documented end to end, with committed snapshots |

## License

Apache-2.0. The bundled IBM Plex Sans font is under the SIL Open Font License; see
[`crates/styleguidist/fonts/OFL.txt`](crates/styleguidist/fonts/OFL.txt).
