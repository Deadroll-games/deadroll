//! Deadroll balance bench: `cargo run -p balance --release`.
//!
//! Presets are read from `tools/balance/presets` (or the folder given as the first argument, or
//! `DEADROLL_PRESETS`). Fights run on `deadroll-sim`, which calls the session program's rules.
mod app;
mod tabs;
mod widgets;

use std::path::PathBuf;

fn presets_dir() -> PathBuf {
    std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("DEADROLL_PRESETS").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("presets"))
}

fn main() -> eframe::Result {
    let root = presets_dir();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Deadroll balance")
            .with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Deadroll balance",
        options,
        Box::new(move |_cc| Ok(Box::new(app::BalanceApp::new(root)))),
    )
}
