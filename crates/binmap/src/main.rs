//! The Binmap application.
//!
//! This binary exists to wire an engine to an interface and to do nothing
//! else. It is the only place that names both `binmap-build` and
//! `binmap-gui`, which is what makes §2.4's rule structural rather than
//! aspirational: the interface crate does not depend on anything that
//! computes, so a view cannot call an analysis even by accident. Keep this
//! file short, and keep that manifest shorter.

use binmap_build::engine::BinmapEngine;
use binmap_core::config::ProjectConfig;
use binmap_core::evidence::ToolInvocation;
use binmap_core::facade::Engine;
use binmap_verify::GatePlan;
use std::sync::Arc;

fn main() -> std::process::ExitCode {
    // N8: not a command-line tool. The one argument this accepts is a project
    // to open, which is the launcher question §11 leaves open — not a command
    // line, and deliberately not growing into one.
    let root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    let config = ProjectConfig::new(&root);
    let gates = GatePlan::new(ToolInvocation::new("cargo", ["build", "--release", "--quiet"]))
        .testing_with(ToolInvocation::new("cargo", ["test", "--release", "--quiet"]));

    let engine = match BinmapEngine::open(config, gates) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("binmap: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };

    // The interface only ever sees this.
    let engine: Arc<dyn Engine> = Arc::new(engine);

    // The project's own name, as the title bar shows it.
    let project = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());

    binmap_gui::app::run(engine, project, Some(root.display().to_string()));
    std::process::ExitCode::SUCCESS
}
