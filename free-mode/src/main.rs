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
        ACCENT, ButtonColors, FestivalUiPlugin, MUTED_COLOR, Selected, TEXT_COLOR, TutorialPage,
        UiFont, button_styled, button_with, hud_text, panel, set_text, tutorial_closed,
    },
};

const GRID: UVec2 = UVec2::new(10016, 6800);
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
    let initial_words = presets
        .iter()
        .find(|p| p.name.contains("고스퍼"))
        .map(|p| pack_words(&grid, &[(&p.pattern, grid.centered_origin(&p.pattern), 0)]));

    App::new()
        .add_plugins(conway_core::festival_default_plugins(
            "생명 게임 — 자유 모드",
        ))
        .add_plugins((
            ConwaySimPlugin(SimConfig {
                size: GRID,
                initial_rate: 10.0,
                initial_view: InitialView::CellsWide(150.0),
                initial_words: if debug::selftest_enabled() {
                    None
                } else {
                    initial_words
                },
                readback: debug::selftest_enabled(),
                ..default()
            }),
            CameraControlPlugin,
            PaintPlugin::default(),
            FestivalUiPlugin::new(tutorial_pages()).open_at_start(true),
            debug::ScreenshotPlugin {
                name: "free_mode".into(),
            },
            debug::SelfTestPlugin(selftest()),
        ))
        .insert_resource(Presets(presets))
        .add_message::<Action>()
        .add_systems(Startup, setup_ui)
        .add_systems(PostStartup, initial_view)
        .add_systems(
            Update,
            (
                keyboard_shortcuts.run_if(tutorial_closed),
                handle_actions,
                handle_dropped_rle,
                update_hud,
            )
                .chain()
                .after(SimSet),
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

fn tutorial_pages() -> Vec<TutorialPage> {
    vec![
        TutorialPage::new(
            "콘웨이의 생명 게임이란?",
            "격자 위의 셀은 '살아있음' 또는 '죽음' 두 상태를 가집니다.\n\
             매 세대마다 모든 셀이 동시에 다음 규칙을 따릅니다.\n\n\
             • 살아있는 셀: 이웃(주변 8칸)이 2개 또는 3개면 살아남고, 아니면 죽습니다.\n\
             • 죽은 셀: 이웃이 정확히 3개면 새로 태어납니다.\n\n\
             단순한 규칙에서 놀랍도록 복잡한 움직임이 나타납니다. 직접 그려서 확인해 보세요!",
        ),
        TutorialPage::new(
            "그리기",
            "• 왼쪽 클릭 / 드래그 : 셀 살리기\n\
             • 오른쪽 클릭 / 드래그 : 셀 지우기\n\
             • 재생 중에도 그릴 수 있습니다.\n\n\
             상단의 '지우기'는 전체를 비우고, '랜덤'은 지금 보이는 영역을 무작위로 채웁니다.",
        ),
        TutorialPage::new(
            "재생과 속도",
            "• 재생 / 정지 : 상단 버튼 또는 Space\n\
             • 한 세대만 진행 : N 키\n\
             • 속도 - / + : [ 와 ] 키 (1 ~ 2048 세대/초)\n\n\
             천천히 보면 규칙이 보이고, 빠르게 돌리면 큰 패턴의 전체 흐름이 보입니다.",
        ),
        TutorialPage::new(
            "프리셋 체험",
            "왼쪽 목록의 프리셋을 누르면 화면 중앙에 그 패턴이 놓이고 카메라가 맞춰집니다.\n\n\
             • 정지 패턴 : 변하지 않는 모양 (블록, 벌집 …)\n\
             • 진동 패턴 : 주기적으로 반복 (깜빡이, 펄서 …)\n\
             • 이동 패턴 : 우주선처럼 움직임 (글라이더, LWSS …)\n\
             • 글라이더 건 : 글라이더를 끝없이 발사\n\
             • 시계 : 실제 시각을 표시하는 초대형 패턴 (빠른 속도 권장)\n\n\
             프리셋 옆 '스탬프'를 고르면 그리드를 클릭한 자리에 그 패턴을 찍을 수 있습니다.\n\
             '펜' 버튼(P)으로 다시 그리기 모드로 돌아옵니다.",
        ),
        TutorialPage::new(
            "화면 이동",
            "• 마우스 휠 : 커서 위치 기준 확대/축소\n\
             • W A S D / 방향키 또는 가운데 버튼 드래그 : 이동\n\
             • F : 그리드 전체 보기\n\n\
             격자는 아주 넓고(10016 x 6800), 가장자리는 반대편과 이어져 있습니다.\n\
             이제 마음껏 실험해 보세요!",
        ),
    ]
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
                hud_text(&font, "자유 모드", 22.0, ACCENT),
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
            side.spawn(hud_text(&font, "프리셋", 20.0, ACCENT));
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
                warn!("파일 읽기 실패({}): {e}", path_buf.display());
                continue;
            }
        };
        match parse_rle(&text) {
            Ok(pattern) => {
                info!(
                    "RLE 로드: {} ({}x{})",
                    path_buf.display(),
                    pattern.width,
                    pattern.height
                );
                let origin = grid.centered_origin(&pattern);
                reset.write(ResetGrid(pack_words(&grid, &[(&pattern, origin, 0)])));
            }
            Err(e) => warn!("RLE 파싱 실패({}): {e}", path_buf.display()),
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
