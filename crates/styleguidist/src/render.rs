//! Headless rendering of examples to PNG snapshots.

use crate::manifest::{
    self as manifest, ExampleEntry, KnobValue, Manifest, VariantEntry, variant_key,
};
use crate::{
    Example, Frame, Knobs, Styleguide,
    knobs::variants,
    preview::{Canvas, PADDING},
};
use anyhow::{Context as _, Result, anyhow, bail};
use gpui::{
    AnyWindowHandle, AppContext as _, Bounds, HeadlessAppContext, PlatformTextSystem, WindowBounds,
    WindowOptions, px, size,
};
use image::RgbaImage;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Logical size of the window an example is laid out in before cropping.
const CANVAS: (f32, f32) = (960., 640.);

pub(crate) struct Renderer<'a> {
    guide: &'a Styleguide,
    text_system: Arc<dyn PlatformTextSystem>,
}

pub(crate) struct Snapshot {
    pub(crate) entry: VariantEntry,
    pub(crate) image: RgbaImage,
}

impl<'a> Renderer<'a> {
    pub(crate) fn new(guide: &'a Styleguide) -> Result<Self> {
        if gpui_platform::current_headless_renderer()?.is_none() {
            bail!(
                "GPUI has no headless renderer on this platform; render snapshots on macOS or Linux"
            );
        }
        Ok(Self {
            guide,
            text_system: gpui_platform::current_platform(true).text_system(),
        })
    }

    fn context(&self) -> HeadlessAppContext {
        let assets: Arc<dyn gpui::AssetSource> =
            self.guide.assets.clone().unwrap_or_else(|| Arc::new(()));
        let mut cx = HeadlessAppContext::with_platform(
            self.text_system.clone(),
            assets,
            gpui_platform::current_headless_renderer,
        );
        cx.update(|cx| self.guide.run_setup(cx));
        cx
    }

    /// Renders every pre-renderable variant of `example` in every theme.
    pub(crate) fn render(&self, example: &Example) -> Result<(ExampleEntry, Vec<Snapshot>)> {
        let mut cx = self.context();
        let mut knob_specs = Vec::new();
        let mut snapshots = Vec::new();
        for theme in &self.guide.themes {
            cx.update(|cx| self.guide.apply_theme(theme, cx));

            // The first render registers the knobs the example reads.
            let discovery = Knobs::default();
            let (image, scale) = self.capture(&mut cx, example, discovery.clone())?;
            knob_specs = discovery.specs();
            let combos = variants(&knob_specs, self.guide.variant_limit);

            for (index, values) in combos.into_iter().enumerate() {
                let (image, scale) = if index == 0 {
                    (image.clone(), scale)
                } else {
                    self.capture(&mut cx, example, Knobs::with_values(values.clone()))?
                };
                let image = match example.fixed_size() {
                    Some(_) => image,
                    None => crop(&image, (PADDING * scale) as u32),
                };
                let ordered: Vec<(&str, &KnobValue)> = knob_specs
                    .iter()
                    .map(|spec| (spec.name.as_str(), &values[&spec.name]))
                    .collect();
                let key = variant_key(ordered);
                // Name files by what differs from the default, so they stay
                // short and stable when knobs are added.
                let changed: Vec<(&str, &KnobValue)> = knob_specs
                    .iter()
                    .filter(|spec| values[&spec.name] != spec.default)
                    .map(|spec| (spec.name.as_str(), &values[&spec.name]))
                    .collect();
                let name = if changed.is_empty() {
                    "default".to_string()
                } else {
                    slug(&variant_key(changed))
                };
                let file = format!("snapshots/{}/{}/{name}.png", example.id(), slug(theme));
                snapshots.push(Snapshot {
                    entry: VariantEntry {
                        theme: theme.to_string(),
                        key,
                        values,
                        is_default: index == 0,
                        width: (image.width() as f32 / scale).round() as u32,
                        height: (image.height() as f32 / scale).round() as u32,
                        scale,
                        hash: format!("{:016x}", fnv1a(image.as_raw())),
                        file,
                    },
                    image,
                });
            }
        }

        let entry = ExampleEntry {
            id: example.id().to_string(),
            title: example.display_title().to_string(),
            component: example.component_name().map(|name| name.to_string()),
            description: example.description_text().to_string(),
            knobs: knob_specs,
            themes: self
                .guide
                .themes
                .iter()
                .map(|theme| theme.to_string())
                .collect(),
            variants: snapshots
                .iter()
                .map(|snapshot| snapshot.entry.clone())
                .collect(),
        };
        Ok((entry, snapshots))
    }

    fn capture(
        &self,
        cx: &mut HeadlessAppContext,
        example: &Example,
        knobs: Knobs,
    ) -> Result<(RgbaImage, f32)> {
        let fixed = example.fixed_size();
        let logical = fixed.map_or(size(px(CANVAS.0), px(CANVAS.1)), |size| {
            size + gpui::size(px(PADDING * 2.), px(PADDING * 2.))
        });
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Default::default(),
                size: logical,
            })),
            focus: false,
            show: false,
            ..Default::default()
        };
        let background = self.guide.background.clone();
        let handle: AnyWindowHandle = cx.update(|cx| {
            let build = example.builder();
            let frame: crate::FrameBuilder = Box::new(move |window, cx| {
                let content = build(knobs, window, cx);
                let canvas = cx.new(|_| Canvas {
                    content,
                    background,
                    fill: fixed.is_some(),
                });
                cx.new(|_| Frame::new(canvas.into()))
            });
            (self.guide.open_window)(options, frame, cx)
        })?;

        // Let asset loads and deferred work settle, then draw twice so layout
        // that depends on a previous frame (measured text, scroll) is final.
        for _ in 0..3 {
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })?;
        }
        let scale = cx.update_window(handle, |_, window, _| window.scale_factor())?;
        let image = cx
            .capture_screenshot(handle)
            .with_context(|| format!("capturing `{}`", example.id()))?;
        cx.update_window(handle, |_, window, _| window.remove_window())?;
        cx.run_until_parked();
        Ok((image, scale))
    }
}

/// Crops to the content's bounding box: every pixel that differs from the
/// canvas color at the top-left corner, plus `margin`.
fn crop(image: &RgbaImage, margin: u32) -> RgbaImage {
    let background = *image.get_pixel(0, 0);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if *pixel != background {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if min_x > max_x {
        // Nothing but canvas: keep a small empty tile.
        return image::imageops::crop_imm(image, 0, 0, margin * 2, margin * 2).to_image();
    }
    let x = min_x.saturating_sub(margin);
    let y = min_y.saturating_sub(margin);
    let right = (max_x + 1 + margin).min(image.width());
    let bottom = (max_y + 1 + margin).min(image.height());
    image::imageops::crop_imm(image, x, y, right - x, bottom - y).to_image()
}

/// A file-name-safe form of a variant key or theme.
fn slug(text: &str) -> String {
    let slug: String = text
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '=' | ',' => c,
            _ => '_',
        })
        .collect();
    if slug.len() <= 96 && slug == text {
        slug
    } else {
        let short: String = slug.chars().take(64).collect();
        format!("{short}-{:08x}", fnv1a(text.as_bytes()) as u32)
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
    })
}

fn selected<'a>(guide: &'a Styleguide, filter: Option<&str>) -> Vec<&'a Example> {
    guide
        .examples
        .iter()
        .filter(|example| filter.is_none_or(|prefix| example.id().starts_with(prefix)))
        .collect()
}

fn read_manifest(dir: &Path) -> Result<Option<Manifest>> {
    let path = dir.join(manifest::FILE);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    Ok(Some(
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?,
    ))
}

/// `styleguide render`: writes snapshots and their manifest under `styleguide/`.
pub(crate) fn render(guide: &Styleguide, filter: Option<&str>) -> Result<()> {
    let dir = guide.output_dir();
    let renderer = Renderer::new(guide)?;
    let mut manifest = read_manifest(&dir)?.unwrap_or_default();
    manifest.version = manifest::VERSION;
    // Drop examples that no longer exist; keep unselected ones as they are.
    manifest.examples.retain(|entry| {
        guide
            .examples
            .iter()
            .any(|example| example.id() == &entry.id)
    });

    for example in selected(guide, filter) {
        let (entry, snapshots) = renderer.render(example)?;
        let example_dir = dir.join("snapshots").join(example.id().as_ref());
        if example_dir.exists() {
            fs::remove_dir_all(&example_dir)?;
        }
        for snapshot in &snapshots {
            let path = dir.join(&snapshot.entry.file);
            fs::create_dir_all(path.parent().unwrap())?;
            snapshot
                .image
                .save(&path)
                .with_context(|| format!("writing {}", path.display()))?;
        }
        eprintln!("rendered {} ({} snapshots)", entry.id, snapshots.len());
        match manifest
            .examples
            .iter_mut()
            .find(|existing| existing.id == entry.id)
        {
            Some(existing) => *existing = entry,
            None => manifest.examples.push(entry),
        }
    }
    let order: Vec<&str> = guide
        .examples
        .iter()
        .map(|example| example.id().as_ref())
        .collect();
    manifest
        .examples
        .sort_by_key(|entry| order.iter().position(|id| *id == entry.id));

    write_outputs(&dir, &manifest)?;
    eprintln!("wrote {}", dir.display());
    Ok(())
}

fn write_outputs(dir: &Path, manifest: &Manifest) -> Result<()> {
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(dir.join(manifest::FILE), format!("{json}\n"))?;
    Ok(())
}

/// `styleguide check`: re-renders and compares against the committed
/// snapshots. Mismatches are written to `target/styleguide-check/`.
pub(crate) fn check(guide: &Styleguide, filter: Option<&str>, tolerance: f64) -> Result<()> {
    let dir = guide.output_dir();
    let manifest = read_manifest(&dir)?.ok_or_else(|| {
        anyhow!(
            "no {} in {}; run `render` first",
            manifest::FILE,
            dir.display()
        )
    })?;
    let renderer = Renderer::new(guide)?;
    let out = PathBuf::from("target/styleguide-check");
    let mut failures = Vec::new();

    for example in selected(guide, filter) {
        let Some(committed) = manifest.example(example.id()) else {
            failures.push(format!("{}: not rendered", example.id()));
            continue;
        };
        let (entry, snapshots) = renderer.render(example)?;
        if entry.knobs != committed.knobs {
            failures.push(format!("{}: knobs changed", example.id()));
        }
        for snapshot in snapshots {
            let label = format!(
                "{} [{}] {}",
                example.id(),
                snapshot.entry.theme,
                snapshot.entry.key
            );
            let Some(expected) = committed.variants.iter().find(|variant| {
                variant.theme == snapshot.entry.theme && variant.key == snapshot.entry.key
            }) else {
                failures.push(format!("{label}: new variant"));
                continue;
            };
            let expected_image = image::open(dir.join(&expected.file))
                .with_context(|| format!("opening {}", expected.file))?
                .to_rgba8();
            let difference = difference(&expected_image, &snapshot.image);
            if difference > tolerance {
                let path = out.join(&snapshot.entry.file);
                fs::create_dir_all(path.parent().unwrap())?;
                snapshot.image.save(&path)?;
                failures.push(format!(
                    "{label}: {:.2}% of pixels differ (actual: {})",
                    difference * 100.,
                    path.display()
                ));
            }
        }
    }

    if failures.is_empty() {
        eprintln!("snapshots match");
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("  {failure}");
        }
        bail!(
            "{} snapshot mismatches; run `render` to accept them",
            failures.len()
        )
    }
}

/// Share of pixels whose channels differ by more than a small anti-aliasing
/// allowance; 1.0 when the sizes differ.
fn difference(expected: &RgbaImage, actual: &RgbaImage) -> f64 {
    if expected.dimensions() != actual.dimensions() {
        return 1.;
    }
    let differing = expected
        .pixels()
        .zip(actual.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0.iter()).any(|(a, b)| a.abs_diff(*b) > 8))
        .count();
    differing as f64 / (expected.width() as f64 * expected.height() as f64).max(1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn crop_keeps_content_and_margin() {
        let mut image = RgbaImage::from_pixel(100, 80, Rgba([255, 255, 255, 255]));
        for x in 40..50 {
            for y in 30..35 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let cropped = crop(&image, 4);
        assert_eq!(cropped.dimensions(), (18, 13));
        assert_eq!(*cropped.get_pixel(4, 4), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn slugs_are_stable_and_file_safe() {
        assert_eq!(
            slug("disabled=true,variant=ghost"),
            "disabled=true,variant=ghost"
        );
        let long = slug(&"label=Hello world/".repeat(10));
        assert!(long.len() < 80 && !long.contains('/') && !long.contains(' '));
        assert_eq!(long, slug(&"label=Hello world/".repeat(10)));
    }
}
