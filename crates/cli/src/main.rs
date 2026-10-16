//! `cargo styleguide`: builds a crate's styleguide example to WebAssembly and
//! writes a static site that runs the gallery in the browser, and reloads the
//! styleguide on source changes during development.

mod watch;

use anyhow::{Context as _, Result, anyhow, bail};
use gpui_styleguidist_manifest::{self as manifest, Manifest, llms};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead as _, BufReader, Write as _},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use watch::Watcher;

const USAGE: &str = "\
usage: cargo styleguide <command> [options]

commands:
  build    compile the styleguide example to wasm and write a static site
  serve    build, then serve the site on localhost; with --watch, rebuild on
           source changes and reload the page
  dev      run the native styleguide, and rebuild and reopen it at the same
           example, theme and knobs on source changes
  llms     write Markdown docs for language models from the rendered manifest
           (styleguide/manifest.json): llms.txt, llms-full.txt and one page
           per component under llms/

options:
  -p, --package NAME   the crate whose styleguide to build (default: the only
                       workspace member with a `styleguide` example)
  --example NAME       the example that calls gpui_styleguidist::main
                       (default: styleguide)
  --release            optimized build with the `release` profile
  --profile NAME       optimized build with this cargo profile, e.g. one with
                       opt-level = 'z' and lto for a smaller download
  --out-dir DIR        site directory, or for llms the directory of its files
                       (default: target/styleguide/<package>); --out also works
  --out-file FILE      llms: write every component page into this one file
                       instead, or to stdout for `-`
  --title TEXT         page title (default: the package name)
  --toolchain NAME     rustup toolchain for the wasm build (default: nightly,
                       which GPUI's wasm scheduler requires; `none` uses the
                       current cargo)
  --port PORT          serve: port (default: 8000)
  --watch              serve: rebuild on source changes and reload the page";

/// Debug GPUI builds exceed wasm-ld's default 1 MiB stack on deep render
/// trees; gpui-kit's web gallery links with the same size.
const STACK_SIZE: &str = "-zstack-size=8388608";

const TARGET: &str = "wasm32-unknown-unknown";

struct Options {
    package: Option<String>,
    example: String,
    release: bool,
    out: Option<PathBuf>,
    out_file: Option<PathBuf>,
    title: Option<String>,
    toolchain: String,
    profile: String,
    port: u16,
    watch: bool,
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
        out_file: None,
        title: None,
        toolchain: "nightly".into(),
        profile: "release".into(),
        port: 8000,
        watch: false,
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
            "--profile" => {
                options.release = true;
                options.profile = value()?;
            }
            "--out" | "--out-dir" => options.out = Some(value()?.into()),
            "--out-file" => options.out_file = Some(value()?.into()),
            "--title" => options.title = Some(value()?),
            "--toolchain" => options.toolchain = value()?,
            "--port" => options.port = value()?.parse()?,
            "--watch" => options.watch = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => bail!("unknown option {other}\n\n{USAGE}"),
        }
    }
    match command.as_str() {
        "build" => build(&options).map(|_| ()),
        "serve" if options.watch => serve_watch(&options),
        "serve" => {
            let site = build(&options)?;
            serve(&site, options.port, None)
        }
        "dev" => dev(&options),
        "llms" => llms(&options),
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
    let target_dir = target_dir(&metadata)?;

    eprintln!(
        "building {package} --example {} for {TARGET}",
        options.example
    );
    let mut cargo = wasm_cargo(&options.toolchain);
    if !options.release {
        // Browsers don't use wasm debug info, and it makes a debug styleguide
        // several times larger to bind and download.
        cargo.env("CARGO_PROFILE_DEV_DEBUG", "0");
    }
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
        cargo.args(["--profile", &options.profile]);
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

    let profile = if options.release {
        options.profile.as_str()
    } else {
        "debug"
    };
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
    fs::write(out.join("index.html"), index_html(&title, options.watch))?;
    // The site serves its LLM docs too, once snapshots have been rendered.
    if let Some(manifest) = read_manifest(&metadata, &package)? {
        write_llms(&manifest, &out)?;
    }
    eprintln!("wrote {}", out.display());
    Ok(out)
}

/// Writes the LLM docs for the package's rendered manifest.
fn llms(options: &Options) -> Result<()> {
    let metadata = cargo_metadata()?;
    let package = select_package(&metadata, options)?;
    let manifest = read_manifest(&metadata, &package)?.with_context(|| {
        format!(
            "{package} has no {}/{}; render it first with `cargo run -p {package} --example {} -- render`",
            manifest::DIR,
            manifest::FILE,
            options.example
        )
    })?;
    match &options.out_file {
        Some(file) if file.as_os_str() == "-" => {
            print!("{}", llms::full(&manifest));
        }
        Some(file) => {
            if let Some(parent) = file
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(file, llms::full(&manifest))?;
            eprintln!("wrote {}", file.display());
        }
        None => {
            let out = match &options.out {
                Some(out) => out.clone(),
                None => target_dir(&metadata)?.join("styleguide").join(&package),
            };
            write_llms(&manifest, &out)?;
            eprintln!("wrote {}", out.join(llms::INDEX).display());
        }
    }
    Ok(())
}

/// `llms.txt`, `llms-full.txt` and `llms/<Component>.md` in `out`.
fn write_llms(manifest: &Manifest, out: &Path) -> Result<()> {
    let pages = out.join(llms::PAGES);
    if pages.exists() {
        fs::remove_dir_all(&pages)?;
    }
    fs::create_dir_all(&pages)?;
    fs::write(out.join(llms::INDEX), llms::index(manifest))?;
    fs::write(out.join(llms::FULL), llms::full(manifest))?;
    for component in llms::components(manifest) {
        fs::write(out.join(component.path()), llms::page(&component))?;
    }
    Ok(())
}

/// The package's rendered manifest, if it has one.
fn read_manifest(metadata: &Value, package: &str) -> Result<Option<Manifest>> {
    let manifest_path = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["name"] == package)
        .and_then(|entry| entry["manifest_path"].as_str())
        .with_context(|| format!("no package named {package}"))?;
    let path = Path::new(manifest_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join(manifest::DIR)
        .join(manifest::FILE);
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(None);
    };
    let manifest =
        serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(manifest))
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

fn index_html(title: &str, watch: bool) -> String {
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let reload = if watch {
        format!("<script>\n{}</script>\n", include_str!("reload.js"))
    } else {
        String::new()
    };
    include_str!("index.html")
        .replace("{{title}}", &title)
        .replace("{{reload}}", &reload)
}

/// The rebuild state `serve --watch` reports to the page at [`RELOAD_PATH`].
#[derive(Default)]
struct Rebuild {
    /// Bumped after every successful rebuild; the page reloads when it changes.
    version: u64,
    building: bool,
    failed: bool,
}

const RELOAD_PATH: &str = "/__styleguide/reload";

/// Serves the site, rebuilding it on source changes. The page polls
/// [`RELOAD_PATH`] and reloads after a successful rebuild; its URL keeps the
/// route, theme and isolated knobs, so it reopens in the same place.
fn serve_watch(options: &Options) -> Result<()> {
    let site = build(options)?;
    let metadata = cargo_metadata()?;
    let watcher = Watcher::new(&metadata, vec![target_dir(&metadata)?, site.clone()])?;
    let state = Arc::new(Mutex::new(Rebuild::default()));
    let port = options.port;
    let server = {
        let (site, state) = (site.clone(), state.clone());
        std::thread::spawn(move || serve(&site, port, Some(state)))
    };
    loop {
        if server.is_finished() {
            return server
                .join()
                .unwrap_or_else(|_| Err(anyhow!("the server stopped")));
        }
        if watcher.changed(Duration::from_millis(500)).is_empty() {
            continue;
        }
        eprintln!("\nsources changed; rebuilding");
        state.lock().unwrap().building = true;
        let started = Instant::now();
        let result = build(options);
        let mut state = state.lock().unwrap();
        state.building = false;
        match result {
            Ok(_) => {
                state.version += 1;
                state.failed = false;
                eprintln!("rebuilt in {:.1?}; reloading the page", started.elapsed());
            }
            Err(error) => {
                state.failed = true;
                eprintln!("error: {error:#}\nthe page keeps the last good build");
            }
        }
    }
}

/// Runs the native styleguide and, on every source change, rebuilds it and
/// reopens it where it was: the gallery records its location in a link file,
/// and the new process opens with `gallery --link`.
fn dev(options: &Options) -> Result<()> {
    let metadata = cargo_metadata()?;
    let package = select_package(&metadata, options)?;
    let target_dir = target_dir(&metadata)?;
    let link_file = target_dir
        .join("styleguide")
        .join(format!("{package}.link"));
    fs::create_dir_all(link_file.parent().unwrap())?;
    fs::remove_file(&link_file).ok();
    let watcher = Watcher::new(&metadata, vec![target_dir])?;

    let mut child = Some(launch(&build_native(options, &package)?, &link_file)?);
    loop {
        if let Some(status) = child
            .as_mut()
            .and_then(|child| child.try_wait().ok().flatten())
        {
            if status.success() {
                // The window was closed.
                return Ok(());
            }
            eprintln!("the styleguide exited ({status}); waiting for changes");
            child = None;
        }
        if watcher.changed(Duration::from_millis(250)).is_empty() {
            continue;
        }
        eprintln!("\nsources changed; rebuilding");
        let started = Instant::now();
        match build_native(options, &package) {
            Ok(executable) => {
                if let Some(mut child) = child.take() {
                    child.kill().ok();
                    child.wait().ok();
                }
                child = Some(launch(&executable, &link_file)?);
                eprintln!("reopened in {:.1?}", started.elapsed());
            }
            Err(error) => {
                eprintln!("error: {error:#}\nthe running styleguide keeps the last good build")
            }
        }
    }
}

/// Builds the styleguide example natively and returns its executable.
fn build_native(options: &Options, package: &str) -> Result<PathBuf> {
    let mut cargo = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cargo.args([
        "build",
        "-p",
        package,
        "--example",
        &options.example,
        "--message-format=json-render-diagnostics",
    ]);
    if options.release {
        cargo.args(["--profile", &options.profile]);
    }
    let output = cargo
        .stderr(Stdio::inherit())
        .output()
        .context("running cargo")?;
    if !output.status.success() {
        bail!("the build failed");
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == options.example.as_str()
        })
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
        .ok_or_else(|| anyhow!("cargo built no `{}` executable", options.example))
}

/// Starts the styleguide at the location in `link_file`, if any, and has it
/// keep that file up to date.
fn launch(executable: &Path, link_file: &Path) -> Result<Child> {
    let mut command = Command::new(executable);
    command
        .arg("gallery")
        .env("STYLEGUIDE_LINK_FILE", link_file);
    if let Ok(link) = fs::read_to_string(link_file)
        && !link.trim().is_empty()
    {
        command.args(["--link", link.trim()]);
    }
    command
        .spawn()
        .with_context(|| format!("starting {}", executable.display()))
}

fn target_dir(metadata: &Value) -> Result<PathBuf> {
    metadata["target_directory"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("cargo metadata has no target_directory"))
}

/// A minimal static file server for previewing the site. With `rebuild`, it
/// also answers the page's reload polls.
fn serve(site: &Path, port: u16, rebuild: Option<Arc<Mutex<Rebuild>>>) -> Result<()> {
    let listener =
        TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("binding port {port}"))?;
    eprintln!("serving {} at http://127.0.0.1:{port}/", site.display());
    for stream in listener.incoming() {
        if let Err(error) = respond(stream?, site, rebuild.as_deref()) {
            eprintln!("request failed: {error:#}");
        }
    }
    Ok(())
}

fn respond(mut stream: TcpStream, site: &Path, rebuild: Option<&Mutex<Rebuild>>) -> Result<()> {
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
    let (status, body, content_type) = match (rebuild, path) {
        (Some(rebuild), RELOAD_PATH) => {
            let rebuild = rebuild.lock().unwrap();
            let body = serde_json::json!({
                "version": rebuild.version,
                "building": rebuild.building,
                "failed": rebuild.failed,
            });
            ("200 OK", body.to_string().into_bytes(), "application/json")
        }
        _ => match fs::read(&file) {
            Ok(body) => ("200 OK", body, content_type(&file)),
            Err(_) => ("404 Not Found", b"not found".to_vec(), "text/plain"),
        },
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
