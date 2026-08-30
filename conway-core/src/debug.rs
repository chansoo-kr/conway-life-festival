use bevy::{
    diagnostic::FrameCount,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

use crate::{
    grid::{GridSize, PendingEdits, pack_words},
    rle::Pattern,
    sim::{Generation, GridSnapshot, SimControl, SimReady, SimStats},
    ui::Tutorial,
};

pub fn screenshot_dir() -> Option<String> {
    std::env::var("CONWAY_SCREENSHOT_DIR")
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn selftest_enabled() -> bool {
    std::env::var("CONWAY_SELFTEST").is_ok_and(|v| v == "1")
}

pub struct ScreenshotPlugin {
    pub name: String,
}

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        let Some(dir) = screenshot_dir() else {
            return;
        };
        let name = self.name.clone();
        app.add_systems(
            Update,
            move |mut commands: Commands,
                  frame: Res<FrameCount>,
                  time: Res<Time<Real>>,
                  tutorial: Option<ResMut<Tutorial>>,
                  mut exit: MessageWriter<AppExit>| {
                if frame.0.is_multiple_of(100) {
                    info!(
                        "[screenshot] frame {} at {:.2}s (avg {:.1} fps)",
                        frame.0,
                        time.elapsed_secs(),
                        frame.0 as f32 / time.elapsed_secs().max(0.001)
                    );
                }
                match frame.0 {
                    90 => {
                        let path = format!("{dir}/{name}_intro.png");
                        info!("screenshot: {path}");
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk(path));
                    }
                    100 => {
                        if let Some(mut t) = tutorial {
                            t.start();
                        }
                    }
                    110 => {
                        let path = format!("{dir}/{name}_tutorial.png");
                        info!("screenshot: {path}");
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk(path));
                    }
                    120 => {
                        if let Some(mut t) = tutorial {
                            t.dismiss();
                        }
                    }
                    700 => {
                        let path = format!("{dir}/{name}_main.png");
                        info!("screenshot: {path}");
                        commands
                            .spawn(Screenshot::primary_window())
                            .observe(save_to_disk(path));
                    }
                    800 => {
                        exit.write(AppExit::Success);
                    }
                    _ => {}
                }
            },
        );
    }
}

pub type Placement = (Pattern, IVec2, u32);

#[derive(Clone)]
pub struct SelfTest {
    pub place: Vec<Placement>,
    pub gens: u64,
    pub expect: Vec<Placement>,
}

#[derive(Resource)]
struct SelfTestState {
    test: SelfTest,
    stage: Stage,
}

#[derive(Debug, Clone, Copy)]
enum Stage {
    WaitReady,
    Placed {
        frame: u32,
    },
    Stepping {
        target: u64,
    },
    Settle {
        frames: u32,
        last_received: u64,
        stable: u32,
    },
    Done,
}

fn expected_words(grid: &GridSize, expect: &[Placement]) -> Vec<u32> {
    let placements: Vec<(&Pattern, IVec2, u32)> =
        expect.iter().map(|(p, o, pl)| (p, *o, *pl)).collect();
    pack_words(grid, &placements)
}

pub struct SelfTestPlugin(pub SelfTest);

impl Plugin for SelfTestPlugin {
    fn build(&self, app: &mut App) {
        if !selftest_enabled() {
            return;
        }
        app.insert_resource(SelfTestState {
            test: self.0.clone(),
            stage: Stage::WaitReady,
        })
        .add_systems(Update, run_selftest);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_selftest(
    frame: Res<FrameCount>,
    ready: Res<SimReady>,
    grid: Res<GridSize>,
    generation: Res<Generation>,
    snapshot: Res<GridSnapshot>,
    stats: Res<SimStats>,
    mut state: ResMut<SelfTestState>,
    mut edits: ResMut<PendingEdits>,
    mut control: ResMut<SimControl>,
    tutorial: Option<ResMut<Tutorial>>,
    mut exit: MessageWriter<AppExit>,
) {
    match state.stage {
        Stage::WaitReady => {
            if !ready.get() || frame.0 < 30 {
                return;
            }
            if let Some(mut t) = tutorial {
                t.dismiss();
            }
            control.paused = true;
            for (pattern, origin, plane) in &state.test.place {
                edits.stamp(&grid, pattern, *origin, *plane);
            }
            info!(
                "[selftest] placed {} patterns, running {} generations",
                state.test.place.len(),
                state.test.gens
            );
            state.stage = Stage::Placed { frame: frame.0 };
        }
        Stage::Placed { frame: placed } => {
            let placed_words = expected_words(&grid, &state.test.place);
            let confirmed = snapshot.is_ready() && snapshot.words == placed_words;
            let waited = frame.0 - placed;
            if confirmed || waited > 600 {
                info!(
                    "[selftest] placement {} (waited {} frames, {} readbacks, generation {})",
                    if confirmed { "OK" } else { "timed out" },
                    waited,
                    snapshot.received,
                    generation.0
                );
                state.stage = Stage::Stepping {
                    target: generation.0 + state.test.gens,
                };
            }
        }
        Stage::Stepping { target } => {
            let spacing: u32 = std::env::var("CONWAY_SELFTEST_SPACING")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .max(1);
            if generation.0 < target {
                if frame.0.is_multiple_of(spacing) {
                    control.step_once = true;
                }
            } else {
                control.paused = true;
                state.stage = Stage::Settle {
                    frames: 0,
                    last_received: snapshot.received,
                    stable: 0,
                };
            }
        }
        Stage::Settle {
            frames,
            last_received,
            stable,
        } => {
            let expected_now = expected_words(&grid, &state.test.expect);
            let arrived = snapshot.received != last_received;
            let matches = snapshot.is_ready() && snapshot.words == expected_now;
            let stable = if arrived && matches {
                stable + 1
            } else if arrived {
                0
            } else {
                stable
            };
            if stable < 3 && frames < 300 {
                state.stage = Stage::Settle {
                    frames: frames + 1,
                    last_received: snapshot.received,
                    stable,
                };
                return;
            }
            info!(
                "[selftest] settled after {frames} frames, {} readbacks, {} generations dispatched / {} render frames / {} app frames",
                snapshot.received,
                stats.dispatched(),
                stats.render_frames(),
                frame.0
            );
            let placements: Vec<(&Pattern, IVec2, u32)> = state
                .test
                .expect
                .iter()
                .map(|(p, o, pl)| (p, *o, *pl))
                .collect();
            let expected = pack_words(&grid, &placements);
            let ok = snapshot.is_ready() && snapshot.words == expected;
            if ok {
                info!(
                    "[selftest] OK: grid matches expectation after {} generations ({} readbacks)",
                    generation.0, snapshot.received
                );
            } else {
                let mut diffs = Vec::new();
                for (i, (a, b)) in snapshot.words.iter().zip(expected.iter()).enumerate() {
                    if a != b {
                        let plane = i as u32 / grid.plane_words();
                        let rem = i as u32 % grid.plane_words();
                        let (y, wx) = (rem / grid.words_x(), rem % grid.words_x());
                        diffs.push(format!(
                            "plane{plane} y={y} x={}..{}: got {a:#010x} want {b:#010x}",
                            wx * 32,
                            wx * 32 + 31
                        ));
                        if diffs.len() >= 12 {
                            break;
                        }
                    }
                }
                error!(
                    "[selftest] FAIL — generation={} readback_ready={} len={} expected_len={}\n{}",
                    generation.0,
                    snapshot.is_ready(),
                    snapshot.words.len(),
                    expected.len(),
                    diffs.join("\n")
                );
            }
            state.stage = Stage::Done;
            exit.write(if ok {
                AppExit::Success
            } else {
                AppExit::error()
            });
        }
        Stage::Done => {}
    }
}
