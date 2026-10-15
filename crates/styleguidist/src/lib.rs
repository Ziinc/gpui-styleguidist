//! A living styleguide for GPUI components, in the spirit of React
//! Styleguidist.
//!
//! - Register **examples** of your components, with **knobs** for their props.
//! - `cargo run --example styleguide` opens the **gallery** natively: every
//!   example, every knob live, every theme.
//! - `cargo styleguide build` compiles the same gallery to WebAssembly and
//!   writes a static site to host anywhere, with `#example-id` deep links.
//! - `cargo run --example styleguide -- render` and `-- check` render examples
//!   headlessly to PNG snapshots for visual regression tests in CI.
//!
//! See the repository README for setup.

mod cli;
mod example;
#[cfg(feature = "gallery")]
pub mod gallery;
mod knobs;
pub mod manifest;
#[cfg(any(feature = "render", feature = "gallery"))]
mod preview;
#[cfg(all(feature = "render", not(target_family = "wasm")))]
mod render;
#[cfg(all(feature = "web", target_family = "wasm"))]
mod web;

pub use cli::main;
pub use example::{Example, Frame, FrameBuilder, Styleguide};
pub use knobs::Knobs;
