use bevy::prelude::*;

use crate::{
    macrocell::parse_macrocell,
    rle::{Pattern, parse_rle, rle_name},
};

#[derive(Clone, Debug)]
pub struct Preset {
    pub name: String,
    pub pattern: Pattern,
    pub rate: Option<f32>,
}

pub const BUILTIN_RLE: &[(&str, &str)] = &[
    ("블록 (정지)", "x = 2, y = 2\n2o$2o!"),
    ("벌집 (정지)", "x = 4, y = 3\nb2o$o2bo$b2o!"),
    ("빵 (정지)", "x = 4, y = 4\nb2o$o2bo$bobo$2bo!"),
    ("보트 (정지)", "x = 3, y = 3\n2o$obo$bo!"),
    ("튜브 (정지)", "x = 3, y = 3\nbo$obo$bo!"),
    ("깜빡이 (진동)", "x = 3, y = 1\n3o!"),
    ("두꺼비 (진동)", "x = 4, y = 2\nb3o$3o!"),
    ("비컨 (진동)", "x = 4, y = 4\n2o$2o$2b2o$2b2o!"),
    (
        "펄서 (진동)",
        "x = 13, y = 13\n2b3o3b3o2b2$o4bobo4bo$o4bobo4bo$o4bobo4bo$2b3o3b3o2b2$2b3o3b3o2b$o4bobo4bo$o4bobo4bo$o4bobo4bo2$2b3o3b3o!",
    ),
    (
        "펜타데카슬론 (진동)",
        "x = 10, y = 3\n2bo4bo2b$2ob4ob2o$2bo4bo!",
    ),
    ("글라이더 (이동)", "x = 3, y = 3\nbob$2bo$3o!"),
    (
        "경량 우주선 LWSS (이동)",
        "x = 5, y = 4\nbo2bo$o4b$o3bo$4o!",
    ),
    (
        "중량 우주선 MWSS (이동)",
        "x = 6, y = 5\n3bo$bo3bo$o$o4bo$5o!",
    ),
    (
        "대형 우주선 HWSS (이동)",
        "x = 7, y = 5\n3b2o$bo4bo$o$o5bo$6o!",
    ),
    ("이터 (포식자)", "x = 4, y = 4\n2o$obo$2bo$2b2o!"),
    ("R-펜토미노 (므두셀라)", "x = 3, y = 3\nb2o$2o$bo!"),
    ("다이하드 (130세대 후 소멸)", "x = 8, y = 3\n6bo$2o$bo3b3o!"),
    ("도토리 (5206세대 성장)", "x = 7, y = 3\nbo$3bo$2o2b3o!"),
    (
        "무한 성장 (10셀)",
        "x = 8, y = 6\n6bo$4bob2o$4bobo$4bo$2bo$obo!",
    ),
    (
        "고스퍼 글라이더 건",
        "x = 36, y = 9\n24bo$22bobo$12b2o6b2o12b2o$11bo3bo4b2o12b2o$2o8bo5bo3b2o$2o8bo3bob2o4bobo$10bo5bo7bo$11bo3bo$12b2o!",
    ),
];

pub fn builtin(name_part: &str) -> Option<Pattern> {
    BUILTIN_RLE
        .iter()
        .find(|(n, _)| n.contains(name_part))
        .and_then(|(_, rle)| parse_rle(rle).ok())
}

pub fn builtin_presets() -> Vec<Preset> {
    BUILTIN_RLE
        .iter()
        .filter_map(|(name, rle)| match parse_rle(rle) {
            Ok(pattern) => Some(Preset {
                name: (*name).to_string(),
                pattern,
                rate: None,
            }),
            Err(e) => {
                error!("failed to parse builtin preset '{name}': {e}");
                None
            }
        })
        .collect()
}

pub fn load_pattern_dir(dir: impl AsRef<std::path::Path>) -> Vec<Preset> {
    let dir = dir.as_ref();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("rle") || e.eq_ignore_ascii_case("mc"))
        })
        .collect();
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    warn!("failed to read pattern file {}: {e}", path.display());
                    return None;
                }
            };
            let is_mc = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("mc"));
            let parsed = if is_mc {
                parse_macrocell(&text)
            } else {
                parse_rle(&text)
            };
            let pattern = match parsed {
                Ok(p) => p,
                Err(e) => {
                    warn!("failed to parse pattern {}: {e}", path.display());
                    return None;
                }
            };
            let name = rle_name(&text).unwrap_or_else(|| {
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "패턴".into())
            });
            let rate = (pattern.width * pattern.height > 1_000_000).then_some(2048.0);
            info!(
                "loaded pattern {} ({}x{})",
                path.display(),
                pattern.width,
                pattern.height
            );
            Some(Preset {
                name,
                pattern,
                rate,
            })
        })
        .collect()
}

pub fn load_presets() -> Vec<Preset> {
    let mut presets = builtin_presets();
    presets.extend(load_pattern_dir(
        std::path::Path::new(&crate::asset_root()).join("patterns"),
    ));
    presets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_presets_parse() {
        assert_eq!(builtin_presets().len(), BUILTIN_RLE.len());
    }

    #[test]
    fn asset_patterns_fit_free_mode_grid() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/patterns");
        let files = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| {
                e.as_ref().is_ok_and(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|x| x == "rle" || x == "mc")
                })
            })
            .count();
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let Some(ext) = path.extension().and_then(|x| x.to_str()) else {
                continue;
            };
            let text = std::fs::read_to_string(&path).unwrap();
            let parsed = match ext {
                "mc" => parse_macrocell(&text),
                "rle" => parse_rle(&text),
                _ => continue,
            };
            if let Err(e) = parsed {
                println!("failed to parse {}: {e}", path.display());
            }
        }
        let presets = load_pattern_dir(&dir);
        for p in &presets {
            println!(
                "{}: {}x{} ({} cells)",
                p.name,
                p.pattern.width,
                p.pattern.height,
                p.pattern.alive_count()
            );
            assert!(
                p.pattern.width <= 16384 && p.pattern.height <= 16384,
                "{} exceeds the free-mode grid",
                p.name
            );
        }
        assert_eq!(presets.len(), files, "some pattern files failed to parse");
    }
}
