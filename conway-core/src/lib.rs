#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod camera;
pub mod debug;
pub mod grid;
pub mod macrocell;
pub mod paint;
pub mod presets;
pub mod qr;
pub mod rle;
pub mod sim;
pub mod ui;

use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
    window::{MonitorSelection, VideoModeSelection, WindowMode},
};

pub fn asset_root() -> String {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let next_to_exe = dir.join("assets");
        if next_to_exe.is_dir() {
            return next_to_exe.to_string_lossy().into_owned();
        }
    }
    let in_crate = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    if in_crate.is_dir() {
        return in_crate.to_string_lossy().into_owned();
    }
    "assets".into()
}

/// 창 모드는 `--window` 인자 또는 `CONWAY_WINDOW` 환경 변수로 정합니다.
/// `fullscreen`(전용 전체화면) · `borderless`(테두리 없는 전체화면) · `1600x900`(창 크기).
/// 부스/키오스크(예: Batocera 포트)에서는 `CONWAY_WINDOW=borderless` 를 권장합니다.
fn window_mode_from_env() -> (WindowMode, Option<(u32, u32)>) {
    let args: Vec<String> = std::env::args().collect();
    let spec = args
        .windows(2)
        .find(|w| w[0] == "--window")
        .map(|w| w[1].clone())
        .or_else(|| std::env::var("CONWAY_WINDOW").ok())
        .unwrap_or_default();
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "windowed" => (WindowMode::Windowed, None),
        "fullscreen" => (
            WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current),
            None,
        ),
        "borderless" => (WindowMode::BorderlessFullscreen(MonitorSelection::Primary), None),
        other => {
            let size = other.split_once('x').and_then(|(w, h)| {
                Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?))
            });
            if size.is_none() {
                warn!("unknown window spec {other:?}; using default window");
            }
            (WindowMode::Windowed, size)
        }
    }
}

pub fn festival_default_plugins(title: &str) -> PluginGroupBuilder {
    let (mode, size) = window_mode_from_env();
    let (w, h) = size.unwrap_or((1600, 900));
    DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (w, h).into(),
                mode,
                title: title.to_string(),
                ..default()
            }),
            ..default()
        })
        .set(AssetPlugin {
            file_path: asset_root(),
            ..default()
        })
}
