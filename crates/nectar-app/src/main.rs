//! Nectar Render : l'atelier de mise en page pour les notes Obsidian.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod journal;
mod pages;
mod panels;
mod theme;
mod worker;

use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result {
    journal::install_panic_hook();
    let mut launch = app::Launch::default();
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--bloc") => launch.select = args.next().map(|a| a.to_string_lossy().into_owned()),
            Some("--onglet") => {
                launch.tab = match args.next().and_then(|a| a.into_string().ok()).as_deref() {
                    Some("style") => Some(app::Tab::Style),
                    Some("verifier" | "vérifier") => Some(app::Tab::Check),
                    _ => Some(app::Tab::Block),
                }
            }
            Some("--clair") => launch.theme = Some(egui::ThemePreference::Light),
            Some("--sombre") => launch.theme = Some(egui::ThemePreference::Dark),
            _ => launch.note = Some(PathBuf::from(arg)),
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Nectar Render")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0])
            .with_drag_and_drop(true)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/nectar-render-256.png"))
                    .unwrap_or_default(),
            ),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native("Nectar Render", options, Box::new(|cc| Ok(Box::new(app::NectarApp::new(cc, launch)))))
}
