use crate::Styleguide;

#[cfg(not(target_family = "wasm"))]
const USAGE: &str = "\
usage: styleguide [command] [options]

commands:
  gallery [--example ID] [--link HASH]
                            open the interactive gallery (default), at a
                            component, or at a link such as
                            '#!/button/primary?theme=dark'
  render [--filter PREFIX]  render snapshots, manifest and docs header to styleguide/
  check [--filter PREFIX] [--tolerance FRACTION]
                            re-render and compare with the committed snapshots
  list                      list example ids";

/// The styleguide binary's entry point: parses the command line and runs the
/// gallery, `render`, `check` or `list`.
///
/// ```ignore
/// // examples/styleguide.rs
/// fn main() {
///     gpui_styleguidist::main(my_crate::styleguide());
/// }
/// ```
///
/// `cargo run --example styleguide -- render`
///
/// Built for wasm32 with the `web` feature, the same entry point runs the
/// gallery in the page; `cargo styleguide build` produces that site.
pub fn main(guide: Styleguide) {
    #[cfg(target_family = "wasm")]
    {
        for error in guide.id_errors() {
            crate::guide::log_error(&error);
        }
        if let Err(error) = gallery(guide, None, None) {
            crate::guide::log_error(&format!("{error:#}"));
        }
    }
    #[cfg(not(target_family = "wasm"))]
    if let Err(error) = run(guide, std::env::args().skip(1).collect()) {
        eprintln!("styleguide: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(not(target_family = "wasm"))]
fn run(guide: Styleguide, args: Vec<String>) -> anyhow::Result<()> {
    let errors = guide.id_errors();
    if !errors.is_empty() {
        anyhow::bail!("{}", errors.join("\n"));
    }
    let mut args = args.into_iter();
    let command = args.next().unwrap_or_else(|| "gallery".into());
    let mut filter = None;
    let mut example = None;
    let mut link = None;
    let mut tolerance = 0.002;
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--filter" => filter = Some(value()?),
            "--example" => example = Some(value()?),
            "--link" => link = Some(value()?),
            "--tolerance" => tolerance = value()?.parse()?,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => anyhow::bail!("unknown option {other}\n\n{USAGE}"),
        }
    }

    match command.as_str() {
        "list" => {
            for example in guide.all_examples() {
                println!("{}", example.id());
            }
            Ok(())
        }
        "render" => render(&guide, filter.as_deref()),
        "check" => check(&guide, filter.as_deref(), tolerance),
        "gallery" => gallery(guide, example, link),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => anyhow::bail!("unknown command {other}\n\n{USAGE}"),
    }
}

#[cfg(all(feature = "render", not(target_family = "wasm")))]
fn render(guide: &Styleguide, filter: Option<&str>) -> anyhow::Result<()> {
    crate::render::render(guide, filter)
}

#[cfg(all(feature = "render", not(target_family = "wasm")))]
fn check(guide: &Styleguide, filter: Option<&str>, tolerance: f64) -> anyhow::Result<()> {
    crate::render::check(guide, filter, tolerance)
}

#[cfg(all(not(feature = "render"), not(target_family = "wasm")))]
fn render(_: &Styleguide, _: Option<&str>) -> anyhow::Result<()> {
    anyhow::bail!("`render` needs gpui-styleguidist's `render` feature")
}

#[cfg(all(not(feature = "render"), not(target_family = "wasm")))]
fn check(_: &Styleguide, _: Option<&str>, _: f64) -> anyhow::Result<()> {
    anyhow::bail!("`check` needs gpui-styleguidist's `render` feature")
}

#[cfg(any(
    all(feature = "gallery", not(target_family = "wasm")),
    all(feature = "web", target_family = "wasm")
))]
fn gallery(guide: Styleguide, example: Option<String>, link: Option<String>) -> anyhow::Result<()> {
    match link {
        Some(link) if example.is_none() => crate::gallery::run_link(guide, &link),
        _ => crate::gallery::run(guide, example),
    }
    Ok(())
}

#[cfg(not(any(
    all(feature = "gallery", not(target_family = "wasm")),
    all(feature = "web", target_family = "wasm")
)))]
fn gallery(_: Styleguide, _: Option<String>, _: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!(if cfg!(target_family = "wasm") {
        "the web gallery needs gpui-styleguidist's `web` feature"
    } else {
        "`gallery` needs gpui-styleguidist's `gallery` feature"
    })
}
