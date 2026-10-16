//! A living styleguide for GPUI components, in the spirit of React
//! Styleguidist.
//!
//! - Register **examples** of your components, with **knobs** for their props.
//! - `cargo run --example styleguide` opens the **gallery** natively: every
//!   example, every knob live, every theme.
//! - `cargo styleguide build` compiles the same gallery to WebAssembly and
//!   writes a static site to host anywhere, with `#example-id` deep links.
//! - `cargo run --example styleguide -- render` renders examples headlessly
//!   to PNG snapshots. [`preview`] embeds them in rustdoc, where the generated
//!   `styleguide/header.html` adds knobs and a live-playground iframe; this
//!   works on docs.rs. `-- check` re-renders them for visual regression tests.
//!
//! See the repository README for setup.

// Without a runtime feature only the builder API and macros are used.
#![cfg_attr(not(any(feature = "gallery", feature = "render")), allow(dead_code))]

mod cli;
pub mod docs;
#[cfg(feature = "gallery")]
pub mod gallery;
mod guide;
mod knobs;
#[cfg(any(feature = "render", feature = "gallery"))]
mod preview;
#[cfg(all(feature = "render", not(target_family = "wasm")))]
mod render;
#[cfg(all(feature = "web", target_family = "wasm"))]
mod web;

pub use cli::main;
pub use gpui_styleguidist_manifest as manifest;
pub use guide::{Example, Frame, FrameBuilder, Styleguide};
pub use knobs::Knobs;

/// Registers an example and captures its source for "View code".
///
/// ```ignore
/// example!("button/primary", |knobs, _, _| {
///     Button::new("save").label(knobs.text("label", "Save"))
/// })
/// .component("Button")
/// ```
///
/// `example!(view "input/empty", |knobs, window, cx| { … })` uses
/// [`Example::view`].
#[cfg(feature = "macros")]
pub use gpui_styleguidist_macros::example;

/// Embeds an example's rendered snapshot in the item's documentation.
///
/// ```ignore
/// #[gpui_styleguidist::preview("button/primary")]
/// pub struct Button { /* … */ }
/// ```
///
/// Options: `theme = "dark"` picks the theme shown first; `variants = false`
/// embeds only the default snapshot. In inner docs, use
/// `#![doc = gpui_styleguidist::preview_html!("button/primary")]`.
#[cfg(feature = "macros")]
pub use gpui_styleguidist_macros::{preview, preview_html};
