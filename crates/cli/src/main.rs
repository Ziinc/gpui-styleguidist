//! `cargo styleguide`: builds a crate's styleguide example to WebAssembly and
//! writes a static site that runs the gallery in the browser.

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead as _, BufReader, Write as _},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
};

const USAGE: &str = "\
usage: cargo styleguide <command> [options]

commands:
  build    compile the styleguide example to wasm and write a static site
  serve    build, then serve the site on localhost

options:
  -p, --package NAME   the crate whose styleguide to build (default: the only
                       workspace member with a `styleguide` example)
  --example NAME       the example that calls gpui_styleguidist::main
                       (default: styleguide)
  --release            optimized build (default for serve: debug)
  --out DIR            site directory (default: target/styleguide/<package>)
  --title TEXT         page title (default: the package name)
  --toolchain NAME     rustup toolchain for the wasm build (default: nightly,
                       which GPUI's wasm scheduler requires; `none` uses the
                       current cargo)
  --port PORT          serve: port (default: 8000)";

/// Debug GPUI builds exceed wasm-ld's default 1 MiB stack on deep render
/// trees; gpui-kit's web gallery links with the same size.
const STACK_SIZE: &str = "-zstack-size=8388608";

const TARGET: &str = "wasm32-unknown-unknown";

struct Options {
    package: Option<String>,
    example: String,
    release: bool,
    out: Option<PathBuf>,
    title: Option<String>,
    toolchain: String,
    port: u16,
}

fn main() {
    // `cargo styleguide …` runs `cargo-styleguide styleguide …`.
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("styleguide") {
        args.remove(0);
    }
    if let Err(error) = run(args) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<()> {
    let mut args = args.into_iter();
    let command = args.next().unwrap_or_default();
    let mut options = Options {
        package: None,
        example: "styleguide".into(),
        release: false,
        out: None,
        title: None,
        toolchain: "nightly".into(),
        port: 8000,
    };
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| anyhow!("{arg} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "-p" | "--package" => options.package = Some(value()?),
            "--example" => options.example = value()?,
            "--release" => options.release = true,
            "--out" => options.out = Some(value()?.into()),
            "--title" => options.title = Some(value()?),
            "--toolchain" => options.toolchain = value()?,
            "--port" => options.port = value()?.parse()?,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => bail!("unknown option {other}\n\n{USAGE}"),
        }
    }
    match command.as_str() {
        "build" => build(&options).map(|_| ()),
        "serve" => {
            let site = build(&options)?;
            serve(&site, options.port)
        }
        "" | "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => bail!("unknown command {other}\n\n{USAGE}"),
    }
}

fn build(options: &Options) -> Result<PathBuf> {
    let metadata = cargo_metadata()?;
    let package = select_package(&metadata, options)?;
    let target_dir = PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or_else(|| anyhow!("cargo metadata has no target_directory"))?,
    );

    eprintln!(
        "building {package} --example {} for {TARGET}",
        options.example
    );
    let mut cargo = wasm_cargo(&options.toolchain);
    cargo.args([
        "rustc",
        "-p",
        &package,
        "--example",
        &options.example,
        "--target",
        TARGET,
    ]);
    if options.release {
        cargo.arg("--release");
    }
    cargo.args(["--", "-C", &format!("link-arg={STACK_SIZE}")]);
    let status = cargo.status().context("running cargo")?;
    if !status.success() {
        bail!(
            "the wasm build failed. Is the target installed (`rustup target add {TARGET} \
             --toolchain {}`), and does {package} enable gpui-styleguidist's `web` feature for \
             wasm targets?",
            options.toolchain
        );
    }

    let profile = if options.release { "release" } else { "debug" };
    let wasm = target_dir
        .join(TARGET)
        .join(profile)
        .join("examples")
        .join(format!("{}.wasm", options.example.replace('-', "_")));
    if !wasm.exists() {
        bail!("expected {} after the build", wasm.display());
    }

    let out = options
        .out
        .clone()
        .unwrap_or_else(|| target_dir.join("styleguide").join(&package));
    fs::create_dir_all(&out)?;
    bindgen(&wasm, &out, &metadata)?;
    let title = options.title.clone().unwrap_or_else(|| package.clone());
    fs::write(out.join("index.html"), index_html(&title))?;
    eprintln!("wrote {}", out.display());
    Ok(out)
}

/// GPUI's wasm scheduler uses unstable `std::arch` APIs, so the wasm build
/// runs on a nightly toolchain through rustup unless told otherwise.
fn wasm_cargo(toolchain: &str) -> Command {
    if toolchain == "none" {
        return Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    }
    let mut command = Command::new("rustup");
    command.args(["run", toolchain, "cargo"]);
    command
}

fn cargo_metadata() -> Result<Value> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["metadata", "--format-version", "1"])
        .output()
        .context("running cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn select_package(metadata: &Value, options: &Options) -> Result<String> {
    if let Some(package) = &options.package {
        return Ok(package.clone());
    }
    let members: Vec<&Value> = metadata["workspace_members"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let candidates: Vec<String> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|package| members.contains(&&package["id"]))
        .filter(|package| {
            package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|target| {
                    target["name"] == options.example.as_str()
                        && target["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "example"))
                })
        })
        .filter_map(|package| package["name"].as_str().map(String::from))
        .collect();
    match candidates.as_slice() {
        [package] => Ok(package.clone()),
        [] => bail!(
            "no workspace member has a `{}` example; pass --example or -p",
            options.example
        ),
        _ => bail!(
            "several packages have a `{}` example: {}; pass -p",
            options.example,
            candidates.join(", ")
        ),
    }
}

/// Runs the `wasm-bindgen` CLI, which must match the `wasm-bindgen` crate
/// version in the lockfile exactly.
fn bindgen(wasm: &Path, out: &Path, metadata: &Value) -> Result<()> {
    let locked = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|package| package["name"] == "wasm-bindgen")
        .and_then(|package| package["version"].as_str())
        .map(String::from);
    let install = match &locked {
        Some(version) => format!("cargo install wasm-bindgen-cli --version {version}"),
        None => "cargo install wasm-bindgen-cli".into(),
    };
    let version = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map_err(|_| anyhow!("wasm-bindgen is not installed; run `{install}`"))?;
    let version = String::from_utf8_lossy(&version.stdout);
    if let Some(locked) = &locked
        && !version.split_whitespace().any(|word| word == locked)
    {
        bail!(
            "{} does not match the locked wasm-bindgen {locked}; run `{install}`",
            version.trim()
        );
    }
    let status = Command::new("wasm-bindgen")
        .arg(wasm)
        .args([
            "--target",
            "web",
            "--no-typescript",
            "--out-name",
            "styleguide",
            "--out-dir",
        ])
        .arg(out)
        .status()?;
    if !status.success() {
        bail!("wasm-bindgen failed");
    }
    Ok(())
}

fn index_html(title: &str) -> String {
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    include_str!("index.html").replace("{{title}}", &title)
}

/// A minimal static file server for previewing the site.
fn serve(site: &Path, port: u16) -> Result<()> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("binding port {port}"))?;
    eprintln!("serving {} at http://127.0.0.1:{port}/", site.display());
    for stream in listener.incoming() {
        if let Err(error) = respond(stream?, site) {
            eprintln!("request failed: {error:#}");
        }
    }
    Ok(())
}

fn respond(mut stream: TcpStream, site: &Path) -> Result<()> {
    let mut request = String::new();
    BufReader::new(&stream).read_line(&mut request)?;
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let relative = path.trim_start_matches('/');
    let file = if relative.is_empty() || relative.split('/').any(|part| part == "..") {
        site.join("index.html")
    } else {
        site.join(relative)
    };
    let (status, body, content_type) = match fs::read(&file) {
        Ok(body) => ("200 OK", body, content_type(&file)),
        Err(_) => ("404 Not Found", b"not found".to_vec(), "text/plain"),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("css") => "text/css",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
