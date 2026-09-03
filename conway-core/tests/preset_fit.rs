use bevy::math::IVec2;
use conway_core::{
    macrocell::parse_macrocell, macrocell::parse_macrocell_cells, presets::load_pattern_dir,
};

const FREE_MODE_GRID: u32 = 16384;

fn bbox(cells: &[IVec2]) -> (IVec2, IVec2) {
    cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| {
        (lo.min(*c), hi.max(*c))
    })
}

#[test]
fn every_preset_fits_free_mode_grid() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/patterns");
    for p in load_pattern_dir(dir) {
        let margin_x = FREE_MODE_GRID as i64 - p.pattern.width as i64;
        let margin_y = FREE_MODE_GRID as i64 - p.pattern.height as i64;
        println!(
            "{:<28} {:>6}x{:<6} margin x={:>6} y={:>6}",
            p.name, p.pattern.width, p.pattern.height, margin_x, margin_y
        );
        assert!(
            margin_x >= 0 && margin_y >= 0,
            "{} ({}x{}) exceeds the free-mode grid",
            p.name,
            p.pattern.width,
            p.pattern.height
        );
    }
}

#[test]
fn macrocell_files_report_cells_outside_core() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/patterns");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "mc") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let raw = parse_macrocell_cells(&text).unwrap();
        let (lo, hi) = bbox(&raw);
        let full = hi - lo + IVec2::ONE;
        let core = parse_macrocell(&text).unwrap();
        let outside = raw.len() - core.alive_count() as usize;
        println!(
            "{:<40} file extent {}x{} cells={}  core {}x{} cells={}  outside core={}",
            path.file_name().unwrap().to_string_lossy(),
            full.x,
            full.y,
            raw.len(),
            core.width,
            core.height,
            core.alive_count(),
            outside
        );
        assert!(core.alive_count() > 0);
    }
}

#[test]
#[ignore]
fn dump_core_cells() {
    let Ok(out) = std::env::var("CONWAY_DUMP_DIR") else {
        return;
    };
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/patterns");
    for name in ["computer_8bit_loizeau", "computer_16bit_display_loizeau"] {
        let text = std::fs::read_to_string(dir.join(format!("{name}.mc"))).unwrap();
        let core = parse_macrocell(&text).unwrap();
        let mut s = String::with_capacity(core.alive_count() as usize * 12);
        for c in core.alive_cells() {
            s.push_str(&format!("{} {}\n", c.x, c.y));
        }
        let path = std::path::Path::new(&out).join(format!("{name}.cells"));
        std::fs::write(&path, s).unwrap();
        println!("wrote {} ({}x{})", path.display(), core.width, core.height);
    }
}
