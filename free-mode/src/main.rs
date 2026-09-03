#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::{prelude::*, window::PrimaryWindow};
use conway_core::{
    camera::{CameraControlPlugin, MainCamera, fit_camera, screen_to_world, world_to_cell},
    debug,
    grid::{GridSize, PendingEdits, pack_words},
    paint::{PaintMode, PaintPlugin, PaintState, PaintTool},
    presets::{Preset, load_presets},
    rle::parse_rle,
    sim::{
        ConwaySimPlugin, Generation, GridView, InitialView, ResetGrid, SimConfig, SimControl,
        SimSet, SimSpeed,
    },
    ui::{
        ButtonColors, FestivalUiPlugin, MUTED_COLOR, Selected, SessionReset, TEXT_COLOR, UiFont,
        UiSet, button_styled, button_with, hud_text, intro_closed, panel, set_text, title_text,
    },
};

#[derive(Resource)]
struct InitialState {
    words: Option<Vec<u32>>,
    rate: f32,
    cells_wide: f32,
}

const GRID: UVec2 = UVec2::new(16384, 16384);
const MAX_STEPS_PER_FRAME: u32 = 256;
const RATES: &[f32] = &[
    1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0, 256.0, 512.0, 1024.0, 2048.0,
];
const TOP_BAR_H: f32 = 60.0;
const LEFT_PANEL_W: f32 = 280.0;
const RANDOM_MAX_CELLS: IVec2 = IVec2::new(600, 340);
const RANDOM_DENSITY: f32 = 0.35;

#[derive(Resource)]
struct Presets(Vec<Preset>);

#[derive(Component)]
struct PresetButton(usize);

#[derive(Component)]
struct PenButton;

#[derive(Component)]
struct PlayButton;

#[derive(Component, Clone, Copy)]
enum Hud {
    Generation,
    Speed,
    Tool,
    Cursor,
}

#[derive(Message, Clone, Debug)]
enum Action {
    TogglePlay,
    Step,
    Clear,
    Random,
    SpeedUp,
    SpeedDown,
    Fit,
    LoadPreset(usize),
    SelectStamp(usize),
    Pen,
}

fn main() -> AppExit {
    let presets = load_presets();
    let grid = GridSize::new(GRID, 1);
    let args: Vec<String> = std::env::args().collect();
    let wanted = args
        .windows(2)
        .find(|w| w[0] == "--preset")
        .map(|w| w[1].clone());
    let initial = presets
        .iter()
        .find(|p| wanted.as_deref().is_some_and(|n| p.name.contains(n)))
        .or_else(|| presets.iter().find(|p| p.name.contains("고스퍼")));
    let initial_words =
        initial.map(|p| pack_words(&grid, &[(&p.pattern, grid.centered_origin(&p.pattern), 0)]));
    let zoom = args
        .windows(2)
        .find(|w| w[0] == "--zoom")
        .and_then(|w| w[1].parse::<f32>().ok());
    let view = match (zoom, initial) {
        (Some(cells), _) => InitialView::CellsWide(cells),
        (None, Some(p)) if wanted.is_some() => {
            InitialView::CellsWide(p.pattern.width as f32 * 1.15)
        }
        _ => InitialView::CellsWide(150.0),
    };
    let initial_rate = initial.and_then(|p| p.rate).unwrap_or(10.0);
    let cells_wide = match view {
        InitialView::CellsWide(c) => c,
        _ => 150.0,
    };

    App::new()
        .add_plugins(conway_core::festival_default_plugins(
            "생명 게임 — 자유 모드",
        ))
        .add_plugins((
            ConwaySimPlugin(SimConfig {
                size: GRID,
                initial_rate,
                max_steps_per_frame: MAX_STEPS_PER_FRAME,
                start_paused: args.iter().any(|a| a == "--paused"),
                initial_view: view,
                initial_words: if debug::selftest_enabled() {
                    None
                } else {
                    initial_words.clone()
                },
                readback: debug::selftest_enabled(),
                ..default()
            }),
            CameraControlPlugin,
            PaintPlugin::default(),
            FestivalUiPlugin::new(
                "자유 모드",
                "격자에 자유롭게 생명을 그리고, 우주선·글라이더 건·시계·컴퓨터 같은 프리셋을 불러와 \
                 마음껏 관찰하고 실험하는 모드입니다. 정해진 목표는 없습니다.",
            ),
            debug::ScreenshotPlugin {
                name: "free_mode".into(),
            },
            debug::SelfTestPlugin(selftest()),
        ))
        .insert_resource(Presets(presets))
        .insert_resource(InitialState {
            words: initial_words,
            rate: initial_rate,
            cells_wide,
        })
        .add_message::<Action>()
        .add_systems(Startup, setup_ui)
        .add_systems(PostStartup, initial_view)
        .add_systems(
            Update,
            (
                keyboard_shortcuts.run_if(intro_closed),
                handle_actions,
                on_session_reset,
                handle_dropped_rle,
                update_hud,
            )
                .chain()
                .after(SimSet)
                .after(UiSet),
        )
        .run()
}

fn selftest() -> debug::SelfTest {
    let glider = parse_rle("x = 3, y = 3\nbob$2bo$3o!").unwrap();
    let blinker = parse_rle("x = 3, y = 1\n3o!").unwrap();
    debug::SelfTest {
        place: vec![
            (glider.clone(), IVec2::new(5000, 3400), 0),
            (blinker.clone(), IVec2::new(5100, 3400), 0),
        ],
        gens: 40,
        expect: vec![
            (glider, IVec2::new(5010, 3410), 0),
            (blinker, IVec2::new(5100, 3400), 0),
        ],
    }
}

fn action_button<'a>(
    parent: &'a mut ChildSpawnerCommands<'_>,
    font: &UiFont,
    label: &str,
    colors: ButtonColors,
    action: Action,
) -> EntityCommands<'a> {
    let mut ec = parent.spawn(button_with(font, label, 17.0, colors));
    ec.observe(
        move |_: On<Pointer<Click>>, mut writer: MessageWriter<Action>| {
            writer.write(action.clone());
        },
    );
    ec
}

fn setup_ui(mut commands: Commands, font: Res<UiFont>, presets: Res<Presets>) {
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: px(TOP_BAR_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: UiRect::axes(px(14), px(8)),
            ..default()
        }))
        .with_children(|bar| {
            bar.spawn((
                title_text(&font, "자유 모드", 21.0, TEXT_COLOR),
                Node {
                    margin: UiRect::right(px(14)),
                    ..default()
                },
            ));
            action_button(
                bar,
                &font,
                "재생 (Space)",
                ButtonColors::accent(),
                Action::TogglePlay,
            )
            .insert(PlayButton);
            action_button(
                bar,
                &font,
                "한 세대 (N)",
                ButtonColors::default(),
                Action::Step,
            );
            action_button(
                bar,
                &font,
                "속도 - ([)",
                ButtonColors::default(),
                Action::SpeedDown,
            );
            action_button(
                bar,
                &font,
                "속도 + (])",
                ButtonColors::default(),
                Action::SpeedUp,
            );
            action_button(
                bar,
                &font,
                "랜덤 (R)",
                ButtonColors::default(),
                Action::Random,
            );
            action_button(
                bar,
                &font,
                "지우기 (C)",
                ButtonColors::danger(),
                Action::Clear,
            );
            action_button(
                bar,
                &font,
                "전체 보기 (F)",
                ButtonColors::default(),
                Action::Fit,
            );
            action_button(bar, &font, "펜 (P)", ButtonColors::default(), Action::Pen)
                .insert((PenButton, Selected));
        });

    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(TOP_BAR_H),
            width: px(LEFT_PANEL_W),
            bottom: px(0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(12)),
            row_gap: px(6),
            ..default()
        }))
        .with_children(|side| {
            side.spawn(title_text(&font, "프리셋", 15.0, MUTED_COLOR));
            side.spawn(hud_text(
                &font,
                "클릭: 중앙에 로드 / 스탬프: 클릭한 곳에 찍기",
                13.0,
                MUTED_COLOR,
            ));
            side.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                overflow: Overflow::scroll_y(),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|list| {
                for (i, preset) in presets.0.iter().enumerate() {
                    list.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|row| {
                        let mut load = row.spawn(button_styled(
                            &font,
                            preset.name.as_str(),
                            15.0,
                            ButtonColors::default(),
                            Node {
                                flex_grow: 1.0,
                                justify_content: JustifyContent::FlexStart,
                                padding: UiRect::axes(px(10), px(5)),
                                ..default()
                            },
                        ));
                        load.observe(move |_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                            w.write(Action::LoadPreset(i));
                        });
                        if preset.pattern.width <= 256 && preset.pattern.height <= 256 {
                            row.spawn((
                                button_with(&font, "스탬프", 13.0, ButtonColors::default()),
                                PresetButton(i),
                            ))
                            .observe(
                                move |_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                                    w.write(Action::SelectStamp(i));
                                },
                            );
                        }
                    });
                }
            })
            .observe(
                |scroll: On<Pointer<Scroll>>,
                 mut q: Query<(&mut ScrollPosition, &ComputedNode)>| {
                    if let Ok((mut pos, node)) = q.get_mut(scroll.entity) {
                        let dy = match scroll.unit {
                            bevy::input::mouse::MouseScrollUnit::Line => scroll.y * 24.0,
                            bevy::input::mouse::MouseScrollUnit::Pixel => scroll.y,
                        };
                        let range = (node.content_size.y - node.size.y).max(0.0)
                            * node.inverse_scale_factor;
                        pos.y = (pos.y - dy).clamp(0.0, range);
                    }
                },
            );
        });

    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: px(LEFT_PANEL_W + 12.0),
            bottom: px(12),
            flex_direction: FlexDirection::Row,
            column_gap: px(22),
            padding: UiRect::axes(px(14), px(8)),
            ..default()
        }))
        .with_children(|hud| {
            hud.spawn((hud_text(&font, "", 16.0, TEXT_COLOR), Hud::Generation));
            hud.spawn((hud_text(&font, "", 16.0, TEXT_COLOR), Hud::Speed));
            hud.spawn((hud_text(&font, "", 16.0, TEXT_COLOR), Hud::Tool));
            hud.spawn((hud_text(&font, "", 16.0, MUTED_COLOR), Hud::Cursor));
        });
}

fn on_session_reset(
    mut resets: MessageReader<SessionReset>,
    mut commands: Commands,
    initial: Res<InitialState>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
    mut tool: ResMut<PaintTool>,
    mut reset: MessageWriter<ResetGrid>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
    preset_buttons: Query<Entity, With<PresetButton>>,
    pen_button: Query<Entity, With<PenButton>>,
) {
    if resets.read().last().is_none() {
        return;
    }
    reset.write(match &initial.words {
        Some(words) => ResetGrid(words.clone()),
        None => ResetGrid::empty(&grid),
    });
    speed.rate = initial.rate;
    control.paused = false;
    control.step_once = false;
    tool.mode = PaintMode::Pen;
    tool.enabled = true;
    let (mut tf, mut projection) = camera.into_inner();
    let scale = initial.cells_wide * view.display_factor / window.width().max(1.0);
    if let Projection::Orthographic(o) = &mut *projection {
        o.scale = scale;
    }
    tf.translation = Vec3::new(-LEFT_PANEL_W * 0.5 * scale, TOP_BAR_H * 0.5 * scale, 0.0);
    for e in &preset_buttons {
        commands.entity(e).remove::<Selected>();
    }
    for e in &pen_button {
        commands.entity(e).insert(Selected);
    }
}

fn initial_view(camera: Single<(&mut Transform, &Projection), With<MainCamera>>) {
    let (mut tf, projection) = camera.into_inner();
    if let Projection::Orthographic(o) = projection {
        tf.translation.x = -LEFT_PANEL_W * 0.5 * o.scale;
        tf.translation.y = TOP_BAR_H * 0.5 * o.scale;
    }
}

fn keyboard_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut writer: MessageWriter<Action>) {
    let map = [
        (KeyCode::Space, Action::TogglePlay),
        (KeyCode::KeyN, Action::Step),
        (KeyCode::KeyC, Action::Clear),
        (KeyCode::KeyR, Action::Random),
        (KeyCode::BracketRight, Action::SpeedUp),
        (KeyCode::BracketLeft, Action::SpeedDown),
        (KeyCode::KeyF, Action::Fit),
        (KeyCode::Home, Action::Fit),
        (KeyCode::KeyP, Action::Pen),
        (KeyCode::Escape, Action::Pen),
    ];
    for (key, action) in map {
        if keys.just_pressed(key) {
            writer.write(action);
        }
    }
}

struct XorShift(u64);
impl XorShift {
    fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_actions(
    mut reader: MessageReader<Action>,
    mut commands: Commands,
    presets: Res<Presets>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
    mut tool: ResMut<PaintTool>,
    mut edits: ResMut<PendingEdits>,
    mut reset: MessageWriter<ResetGrid>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
    preset_buttons: Query<(Entity, &PresetButton)>,
    pen_button: Query<Entity, With<PenButton>>,
) {
    let (mut cam_tf, mut projection) = camera.into_inner();
    let viewport = Vec2::new(window.width(), window.height());

    for action in reader.read() {
        match action {
            Action::TogglePlay => control.paused = !control.paused,
            Action::Step => {
                control.paused = true;
                control.step_once = true;
            }
            Action::Clear => {
                reset.write(ResetGrid::empty(&grid));
            }
            Action::Random => {
                let scale = match &*projection {
                    Projection::Orthographic(o) => o.scale,
                    _ => 1.0,
                };
                let cam = cam_tf.translation.truncate();
                let top_left = world_to_cell(
                    screen_to_world(Vec2::new(LEFT_PANEL_W, TOP_BAR_H), viewport, cam, scale),
                    &grid,
                    view.display_factor,
                );
                let bottom_right = world_to_cell(
                    screen_to_world(viewport, viewport, cam, scale),
                    &grid,
                    view.display_factor,
                );
                let center = (top_left + bottom_right) / 2;
                let half = ((bottom_right - top_left) / 2).min(RANDOM_MAX_CELLS / 2);
                let min = (center - half).max(IVec2::ZERO);
                let max = (center + half).min(grid.size.as_ivec2() - IVec2::ONE);
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0x9E37_79B9_7F4A_7C15)
                    | 1;
                let mut rng = XorShift(seed);
                for y in min.y..=max.y {
                    for x in min.x..=max.x {
                        let alive = rng.next_f32() < RANDOM_DENSITY;
                        edits.set_cell(&grid, IVec2::new(x, y), alive.then_some(0));
                    }
                }
            }
            Action::SpeedUp | Action::SpeedDown => {
                let idx = RATES
                    .iter()
                    .position(|r| (*r - speed.rate).abs() < 0.01)
                    .unwrap_or_else(|| {
                        RATES
                            .iter()
                            .position(|r| *r > speed.rate)
                            .unwrap_or(RATES.len() - 1)
                    });
                let new_idx = if matches!(action, Action::SpeedUp) {
                    (idx + 1).min(RATES.len() - 1)
                } else {
                    idx.saturating_sub(1)
                };
                if RATES[new_idx] != speed.rate {
                    speed.rate = RATES[new_idx];
                }
            }
            Action::Fit => fit_camera(
                &mut cam_tf,
                &mut projection,
                &grid,
                &view,
                &window,
                LEFT_PANEL_W,
                TOP_BAR_H,
            ),
            Action::LoadPreset(i) => {
                let Some(preset) = presets.0.get(*i) else {
                    continue;
                };
                let origin = grid.centered_origin(&preset.pattern);
                reset.write(ResetGrid(pack_words(
                    &grid,
                    &[(&preset.pattern, origin, 0)],
                )));
                if let Some(rate) = preset.rate {
                    speed.rate = rate;
                }
                let avail =
                    Vec2::new(viewport.x - LEFT_PANEL_W, viewport.y - TOP_BAR_H).max(Vec2::ONE);
                let pw = (preset.pattern.width as f32).max(60.0) * view.display_factor;
                let ph = (preset.pattern.height as f32).max(34.0) * view.display_factor;
                let scale = (pw / avail.x).max(ph / avail.y) * 1.25;
                if let Projection::Orthographic(o) = &mut *projection {
                    o.scale = scale;
                }
                cam_tf.translation =
                    Vec3::new(-LEFT_PANEL_W * 0.5 * scale, TOP_BAR_H * 0.5 * scale, 0.0);
            }
            Action::SelectStamp(i) => {
                let Some(preset) = presets.0.get(*i) else {
                    continue;
                };
                tool.mode = PaintMode::Stamp {
                    name: preset.name.clone(),
                    pattern: preset.pattern.clone(),
                };
                for (e, b) in &preset_buttons {
                    if b.0 == *i {
                        commands.entity(e).insert(Selected);
                    } else {
                        commands.entity(e).remove::<Selected>();
                    }
                }
                for e in &pen_button {
                    commands.entity(e).remove::<Selected>();
                }
            }
            Action::Pen => {
                if !matches!(tool.mode, PaintMode::Pen) {
                    tool.mode = PaintMode::Pen;
                }
                for (e, _) in &preset_buttons {
                    commands.entity(e).remove::<Selected>();
                }
                for e in &pen_button {
                    commands.entity(e).insert(Selected);
                }
            }
        }
    }
}

fn handle_dropped_rle(
    mut drops: MessageReader<FileDragAndDrop>,
    grid: Res<GridSize>,
    mut reset: MessageWriter<ResetGrid>,
) {
    for event in drops.read() {
        let FileDragAndDrop::DroppedFile { path_buf, .. } = event else {
            continue;
        };
        let text = match std::fs::read_to_string(path_buf) {
            Ok(t) => t,
            Err(e) => {
                warn!("failed to read file {}: {e}", path_buf.display());
                continue;
            }
        };
        match parse_rle(&text) {
            Ok(pattern) => {
                info!(
                    "loaded RLE {} ({}x{})",
                    path_buf.display(),
                    pattern.width,
                    pattern.height
                );
                let origin = grid.centered_origin(&pattern);
                reset.write(ResetGrid(pack_words(&grid, &[(&pattern, origin, 0)])));
            }
            Err(e) => warn!("failed to parse RLE {}: {e}", path_buf.display()),
        }
    }
}

fn update_hud(
    generation: Res<Generation>,
    speed: Res<SimSpeed>,
    control: Res<SimControl>,
    tool: Res<PaintTool>,
    grid: Res<GridSize>,
    paint_state: Res<PaintState>,
    mut hud: Query<(&Hud, &mut Text)>,
    play_button: Query<&Children, With<PlayButton>>,
    mut texts: Query<&mut Text, Without<Hud>>,
) {
    for (kind, mut text) in &mut hud {
        let s = match kind {
            Hud::Generation => format!("세대 {}", generation.0),
            Hud::Speed => format!(
                "속도 {:.0} 세대/초{}",
                speed.rate,
                if control.paused { "  (정지)" } else { "" }
            ),
            Hud::Tool => match &tool.mode {
                PaintMode::Pen => "도구: 펜".to_string(),
                PaintMode::Stamp { name, .. } => format!("도구: 스탬프 — {name}"),
            },
            Hud::Cursor => match paint_state.hover {
                Some(c) if grid.contains(c) => format!("셀 ({}, {})", c.x, c.y),
                _ => String::new(),
            },
        };
        set_text(&mut text, s);
    }
    for children in &play_button {
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                set_text(
                    &mut text,
                    if control.paused {
                        "재생 (Space)"
                    } else {
                        "정지 (Space)"
                    },
                );
            }
        }
    }
}
