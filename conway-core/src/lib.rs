#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod camera;
pub mod debug;
pub mod grid;
pub mod paint;
pub mod presets;
pub mod rle;
pub mod sim;
pub mod ui;

use bevy::{app::PluginGroupBuilder, prelude::*};

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

pub fn festival_default_plugins(title: &str) -> PluginGroupBuilder {
    DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1600, 900).into(),
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
