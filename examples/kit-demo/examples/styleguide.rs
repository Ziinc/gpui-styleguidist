//! `cargo run -p kit-demo --example styleguide` opens the gallery;
//! `-- render` writes `styleguide/`; `-- check` compares against it.
fn main() {
    gpui_styleguidist::main(kit_demo::styleguide());
}
