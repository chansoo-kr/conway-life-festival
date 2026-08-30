#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;
use conway_core::{
    camera::FitCameraPlugin,
    debug,
    grid::{CpuGrid, GridSize, PendingEdits},
    paint::{PaintMode, PaintPlugin, PaintRequest, PaintSet, PaintTool},
    presets::builtin,
    rle::Pattern,
    sim::{
        ConwaySimPlugin, Generation, GridColors, GridSnapshot, GridView, InitialView, ResetGrid,
        SimConfig, SimControl, SimSet, SimSpeed,
    },
    ui::{
        ACCENT, ButtonColors, FestivalUiPlugin, MUTED_COLOR, Selected, StepGoal, TEXT_COLOR,
        Tutorial, TutorialFinished, TutorialStep, UiFont, button_styled, button_with, hud_text,
        intro_closed, panel, set_text,
    },
};

const GRID: UVec2 = UVec2::new(160, 90);
const TOP_BAR_H: f32 = 64.0;
const LEFT_PANEL_W: f32 = 250.0;
const SETUP_SECS: f64 = 60.0;
const BUDGET: u32 = 60;
const BATTLE_GENS: u64 = 600;
const BATTLE_RATE: f32 = 20.0;
const FAST_RATE: f32 = 240.0;
const FINISH_FRAMES: u32 = 8;

const P1_COLOR: Color = Color::srgb(0.35, 0.85, 0.55);
const P2_COLOR: Color = Color::srgb(0.95, 0.45, 0.35);
const PLAYER_NAMES: [&str; 2] = ["플레이어 1 (초록)", "플레이어 2 (주황)"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Setup {
        player: u32,
        deadline: f64,
    },
    Battle,
    Finishing {
        frames: u32,
        received_at_pause: u64,
    },
    Result {
        winner: Option<u32>,
        counts: [u32; 2],
    },
}

#[derive(Resource)]
struct Battle {
    phase: Phase,
    placed: [u32; 2],
    mirror: CpuGrid,
    flip_h: bool,
    flip_v: bool,
    arsenal: usize,
    message: String,
    message_until: f64,
    live: [u32; 2],
}

impl Battle {
    fn notify(&mut self, now: f64, msg: impl Into<String>) {
        self.message = msg.into();
        self.message_until = now + 2.5;
    }
}

#[derive(Resource)]
struct Arsenal(Vec<(String, Pattern)>);

fn build_arsenal() -> Vec<(String, Pattern)> {
    let mut list: Vec<(String, Pattern)> = vec![("펜 (직접 그리기)".into(), Pattern::empty(1, 1))];
    let mut add = |label: &str, key: &str, flip_h: bool| {
        if let Some(p) = builtin(key) {
            let p = if flip_h { p.flip_h() } else { p };
            list.push((format!("{label} ({}셀)", p.alive_count()), p));
        }
    };
    add("글라이더", "글라이더 (이동)", false);
    add("경량 우주선", "LWSS", true);
    add("중량 우주선", "MWSS", true);
    add("R-펜토미노", "R-펜토미노", false);
    add("도토리", "도토리", false);
    add("펄서", "펄서", false);
    add("글라이더 건", "고스퍼", false);
    list
}

#[derive(Component)]
struct ArsenalButton(usize);

#[derive(Component)]
struct FlipHButton;

#[derive(Component)]
struct FlipVButton;

#[derive(Component)]
struct ReadyButton;

#[derive(Component)]
struct FastButton;

#[derive(Component)]
struct TerritoryTint(u32);

#[derive(Component)]
struct ResultPanel;

#[derive(Component, Clone, Copy)]
enum Hud {
    Phase,
    Timer,
    Budget,
    Message,
    Score(u32),
    ScoreBar(u32),
    ResultTitle,
    ResultDetail,
}

#[derive(Message, Clone, Copy, Debug)]
enum Action {
    Ready,
    Fast,
    Restart,
    Arsenal(usize),
    FlipH,
    FlipV,
}

fn main() -> AppExit {
    let grid = GridSize::new(GRID, 2);
    App::new()
        .add_plugins(conway_core::festival_default_plugins(
            "생명 게임 — 1 vs 1 대전",
        ))
        .add_plugins((
            ConwaySimPlugin(SimConfig {
                size: GRID,
                players: 2,
                initial_rate: BATTLE_RATE,
                start_paused: true,
                readback: std::env::var("CONWAY_NO_READBACK").is_err(),
                initial_view: InitialView::FitGrid {
                    left_margin: LEFT_PANEL_W,
                    top_margin: TOP_BAR_H,
                },
                colors: GridColors {
                    p1: P1_COLOR.into(),
                    p2: P2_COLOR.into(),
                    ..default()
                },
                ..default()
            }),
            PaintPlugin { auto_apply: false },
            FitCameraPlugin {
                left_margin: LEFT_PANEL_W,
                top_margin: TOP_BAR_H,
            },
            FestivalUiPlugin::new(
                "1 vs 1 대전",
                "두 사람이 차례로 자기 진영에 생명을 배치한 뒤, 600세대 동안 벌어지는 생존 경쟁에서 \
                 더 많은 셀을 남기는 쪽이 이기는 대전 모드입니다. 새로 태어나는 셀은 부모의 다수 색을 따릅니다.",
                tutorial_steps(),
            )
            .with_rules("왼쪽 초록 진영 안에서 "),
            debug::ScreenshotPlugin {
                name: "battle".into(),
            },
            debug::SelfTestPlugin(selftest()),
        ))
        .insert_resource(Arsenal(build_arsenal()))
        .insert_resource(Battle {
            phase: Phase::Setup {
                player: 0,
                deadline: f64::MAX,
            },
            placed: [0, 0],
            mirror: CpuGrid::new(grid),
            flip_h: false,
            flip_v: false,
            arsenal: 0,
            message: String::new(),
            message_until: 0.0,
            live: [0, 0],
        })
        .add_message::<Action>()
        .add_systems(Startup, (setup_ui, setup_territory))
        .add_systems(
            Update,
            (
                keyboard_shortcuts.run_if(intro_closed),
                on_tutorial_finished,
                handle_actions,
                handle_paint,
                tick_phase,
                sync_tool,
                update_territory,
                update_hud,
            )
                .chain()
                .after(SimSet)
                .after(PaintSet),
        )
        .run()
}

fn selftest() -> debug::SelfTest {
    let glider = conway_core::rle::parse_rle("x = 3, y = 3\nbob$2bo$3o!").unwrap();
    let two = conway_core::rle::parse_rle("x = 2, y = 1\n2o!").unwrap();
    let one = conway_core::rle::parse_rle("x = 1, y = 1\no!").unwrap();
    let blinker = conway_core::rle::parse_rle("x = 3, y = 1\n3o!").unwrap();
    debug::SelfTest {
        place: vec![
            (glider.clone(), IVec2::new(20, 20), 0),
            (glider.clone(), IVec2::new(120, 40), 1),
            (two, IVec2::new(80, 10), 0),
            (one, IVec2::new(82, 10), 1),
        ],
        gens: 40,
        expect: vec![
            (glider.clone(), IVec2::new(30, 30), 0),
            (glider, IVec2::new(130, 50), 1),
            (blinker, IVec2::new(80, 10), 0),
        ],
    }
}

fn setup_ui(mut commands: Commands, font: Res<UiFont>, arsenal: Res<Arsenal>) {
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: px(TOP_BAR_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(14),
            padding: UiRect::axes(px(14), px(8)),
            ..default()
        }))
        .with_children(|bar| {
            bar.spawn(hud_text(&font, "1 vs 1 대전", 22.0, ACCENT));
            bar.spawn((hud_text(&font, "", 18.0, TEXT_COLOR), Hud::Phase));
            bar.spawn((hud_text(&font, "", 18.0, TEXT_COLOR), Hud::Timer));
            bar.spawn((hud_text(&font, "", 18.0, TEXT_COLOR), Hud::Budget));
            bar.spawn((
                hud_text(&font, "", 16.0, Color::srgb(1.0, 0.85, 0.4)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                Hud::Message,
            ));
            bar.spawn((
                button_with(&font, "준비 완료 (Enter)", 17.0, ButtonColors::accent()),
                ReadyButton,
            ))
            .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                w.write(Action::Ready);
            });
            bar.spawn((
                button_with(&font, "빨리 감기 (F)", 17.0, ButtonColors::default()),
                FastButton,
            ))
            .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                w.write(Action::Fast);
            });
            bar.spawn(button_styled(
                &font,
                "다시 시작",
                17.0,
                ButtonColors::danger(),
                Node {
                    margin: UiRect::right(px(150)),
                    ..default()
                },
            ))
            .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                w.write(Action::Restart);
            });
        });

    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(TOP_BAR_H),
            bottom: px(0),
            width: px(LEFT_PANEL_W),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(14)),
            row_gap: px(8),
            ..default()
        }))
        .with_children(|side| {
            side.spawn(hud_text(&font, "점수", 18.0, ACCENT));
            for p in 0..2u32 {
                let color = if p == 0 { P1_COLOR } else { P2_COLOR };
                side.spawn((hud_text(&font, "", 15.0, color), Hud::Score(p)));
                side.spawn((
                    Node {
                        width: percent(100),
                        height: px(12),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.15, 0.19)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: percent(0),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        BackgroundColor(color),
                        Hud::ScoreBar(p),
                    ));
                });
            }

            side.spawn((
                hud_text(&font, "무기고", 18.0, ACCENT),
                Node {
                    margin: UiRect::top(px(12)),
                    ..default()
                },
            ));
            side.spawn(hud_text(
                &font,
                "숫자 키 1~8 · 클릭한 자리에 찍기",
                13.0,
                MUTED_COLOR,
            ));
            for (i, (name, _)) in arsenal.0.iter().enumerate() {
                let mut b = side.spawn((
                    button_styled(
                        &font,
                        format!("{}. {name}", i + 1),
                        15.0,
                        ButtonColors::default(),
                        Node {
                            justify_content: JustifyContent::FlexStart,
                            padding: UiRect::axes(px(10), px(5)),
                            ..default()
                        },
                    ),
                    ArsenalButton(i),
                ));
                if i == 0 {
                    b.insert(Selected);
                }
                b.observe(move |_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::Arsenal(i));
                });
            }
            side.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(8),
                margin: UiRect::top(px(8)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    button_with(&font, "좌우 반전 (H)", 14.0, ButtonColors::default()),
                    FlipHButton,
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::FlipH);
                });
                row.spawn((
                    button_with(&font, "상하 반전 (V)", 14.0, ButtonColors::default()),
                    FlipVButton,
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::FlipV);
                });
            });
        });

    commands
        .spawn((
            ResultPanel,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            Interaction::None,
            GlobalZIndex(50),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(14),
                    padding: UiRect::all(px(32)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.11, 0.12, 0.16)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                Interaction::None,
            ))
            .with_children(|card| {
                card.spawn((hud_text(&font, "", 40.0, ACCENT), Hud::ResultTitle));
                card.spawn((
                    hud_text(&font, "", 20.0, TEXT_COLOR),
                    TextLayout::justify(Justify::Center),
                    Hud::ResultDetail,
                ));
                card.spawn(button_with(
                    &font,
                    "다시 시작 (R)",
                    20.0,
                    ButtonColors::accent(),
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::Restart);
                });
            });
        });
}

fn setup_territory(mut commands: Commands, grid: Res<GridSize>, view: Res<GridView>) {
    let world = grid.size.as_vec2() * view.display_factor;
    for p in 0..2u32 {
        let x = if p == 0 {
            -world.x * 0.25
        } else {
            world.x * 0.25
        };
        commands.spawn((
            TerritoryTint(p),
            Sprite::from_color(Color::NONE, Vec2::new(world.x * 0.5, world.y)),
            Transform::from_xyz(x, 0.0, 1.0),
        ));
    }
    commands.spawn((
        Sprite::from_color(
            Color::srgba(1.0, 1.0, 1.0, 0.55),
            Vec2::new(view.display_factor * 0.4, world.y),
        ),
        Transform::from_xyz(0.0, 0.0, 2.0),
    ));
}

fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    battle: Res<Battle>,
    mut writer: MessageWriter<Action>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        writer.write(Action::Ready);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        writer.write(Action::Fast);
    }
    if keys.just_pressed(KeyCode::KeyR) && matches!(battle.phase, Phase::Result { .. }) {
        writer.write(Action::Restart);
    }
    if keys.just_pressed(KeyCode::KeyH) {
        writer.write(Action::FlipH);
    }
    if keys.just_pressed(KeyCode::KeyV) {
        writer.write(Action::FlipV);
    }
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    for (i, key) in digits.iter().enumerate() {
        if keys.just_pressed(*key) {
            writer.write(Action::Arsenal(i));
        }
    }
}

fn advance_setup(battle: &mut Battle, now: f64, control: &mut SimControl, speed: &mut SimSpeed) {
    match battle.phase {
        Phase::Setup { player: 0, .. } => {
            battle.phase = Phase::Setup {
                player: 1,
                deadline: now + SETUP_SECS,
            };
            battle.arsenal = 0;
            battle.flip_h = false;
            battle.flip_v = false;
            battle.notify(now, "플레이어 2 차례! 오른쪽 진영에 배치하세요.");
        }
        Phase::Setup { .. } => {
            battle.phase = Phase::Battle;
            battle.arsenal = 0;
            control.paused = false;
            speed.rate = BATTLE_RATE;
            battle.notify(now, "전투 시작!");
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_actions(
    mut reader: MessageReader<Action>,
    time: Res<Time>,
    grid: Res<GridSize>,
    arsenal: Res<Arsenal>,
    mut battle: ResMut<Battle>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
    mut reset: MessageWriter<ResetGrid>,
    mut tutorial: ResMut<Tutorial>,
) {
    let now = time.elapsed_secs_f64();
    for action in reader.read() {
        match action {
            Action::Ready => {
                tutorial.complete("ready");
                if matches!(battle.phase, Phase::Setup { .. }) {
                    advance_setup(&mut battle, now, &mut control, &mut speed);
                }
            }
            Action::Fast => {
                if matches!(battle.phase, Phase::Battle) {
                    speed.rate = if speed.rate >= FAST_RATE {
                        BATTLE_RATE
                    } else {
                        FAST_RATE
                    };
                }
            }
            Action::Restart => {
                reset.write(ResetGrid::empty(&grid));
                battle.mirror.clear();
                battle.placed = [0, 0];
                battle.live = [0, 0];
                battle.flip_h = false;
                battle.flip_v = false;
                battle.arsenal = 0;
                battle.phase = Phase::Setup {
                    player: 0,
                    deadline: now + SETUP_SECS,
                };
                control.paused = true;
                speed.rate = BATTLE_RATE;
                battle.notify(now, "새 경기! 플레이어 1부터 배치하세요.");
            }
            Action::Arsenal(i) => {
                if *i < arsenal.0.len() && battle.arsenal != *i {
                    battle.arsenal = *i;
                }
            }
            Action::FlipH => battle.flip_h = !battle.flip_h,
            Action::FlipV => battle.flip_v = !battle.flip_v,
        }
    }
}

fn handle_paint(
    mut reader: MessageReader<PaintRequest>,
    time: Res<Time>,
    grid: Res<GridSize>,
    mut battle: ResMut<Battle>,
    mut edits: ResMut<PendingEdits>,
) {
    let Phase::Setup { player, .. } = battle.phase else {
        reader.clear();
        return;
    };
    let now = time.elapsed_secs_f64();
    let half = grid.size.x as i32 / 2;
    let own = |c: &IVec2| if player == 0 { c.x < half } else { c.x >= half };
    let p = player as usize;

    for req in reader.read() {
        if req.stamp {
            if req.cells.iter().any(|c| !own(c)) {
                battle.notify(now, "자기 진영 안에만 놓을 수 있어요.");
                continue;
            }
            let new_cells: Vec<IVec2> = req
                .cells
                .iter()
                .copied()
                .filter(|c| !battle.mirror.get(player, *c))
                .collect();
            if battle.placed[p] + new_cells.len() as u32 > BUDGET {
                let msg = format!(
                    "셀 예산 부족: {}셀 필요, {}셀 남음",
                    new_cells.len(),
                    BUDGET - battle.placed[p]
                );
                battle.notify(now, msg);
                continue;
            }
            for c in new_cells {
                battle.mirror.set(player, c, true);
                edits.set_cell(&grid, c, Some(player));
                battle.placed[p] += 1;
            }
        } else if req.alive {
            for c in &req.cells {
                if !own(c) {
                    battle.notify(now, "자기 진영 안에만 놓을 수 있어요.");
                    continue;
                }
                if battle.mirror.get(player, *c) {
                    continue;
                }
                if battle.placed[p] >= BUDGET {
                    battle.notify(
                        now,
                        "셀 예산을 모두 썼어요. 오른쪽 클릭으로 되돌릴 수 있어요.",
                    );
                    break;
                }
                battle.mirror.set(player, *c, true);
                edits.set_cell(&grid, *c, Some(player));
                battle.placed[p] += 1;
            }
        } else {
            for c in &req.cells {
                if battle.mirror.get(player, *c) {
                    battle.mirror.set(player, *c, false);
                    edits.set_cell(&grid, *c, None);
                    battle.placed[p] = battle.placed[p].saturating_sub(1);
                }
            }
        }
    }
}

fn on_tutorial_finished(
    mut finished: MessageReader<TutorialFinished>,
    mut actions: MessageWriter<Action>,
) {
    if finished.read().last().is_some() {
        actions.write(Action::Restart);
    }
}

fn tick_phase(
    time: Res<Time>,
    grid: Res<GridSize>,
    generation: Res<Generation>,
    snapshot: Res<GridSnapshot>,
    tutorial: Res<conway_core::ui::Tutorial>,
    mut battle: ResMut<Battle>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
) {
    let now = time.elapsed_secs_f64();
    match battle.phase {
        Phase::Setup { player, deadline } => {
            if deadline == f64::MAX {
                if !tutorial.is_showing() {
                    battle.phase = Phase::Setup {
                        player,
                        deadline: now + SETUP_SECS,
                    };
                }
            } else if now >= deadline {
                advance_setup(&mut battle, now, &mut control, &mut speed);
            }
        }
        Phase::Battle => {
            if snapshot.is_ready() {
                let live = [snapshot.count(&grid, 0), snapshot.count(&grid, 1)];
                if live != battle.live {
                    battle.live = live;
                }
            }
            if generation.0 >= BATTLE_GENS {
                control.paused = true;
                battle.phase = Phase::Finishing {
                    frames: 0,
                    received_at_pause: snapshot.received,
                };
            }
        }
        Phase::Finishing {
            frames,
            received_at_pause,
        } => {
            let settled = frames >= FINISH_FRAMES
                && (snapshot.received >= received_at_pause + 3 || frames > 180);
            if !settled {
                battle.phase = Phase::Finishing {
                    frames: frames + 1,
                    received_at_pause,
                };
            } else {
                let counts = [snapshot.count(&grid, 0), snapshot.count(&grid, 1)];
                let winner = match counts[0].cmp(&counts[1]) {
                    std::cmp::Ordering::Greater => Some(0),
                    std::cmp::Ordering::Less => Some(1),
                    std::cmp::Ordering::Equal => None,
                };
                battle.live = counts;
                battle.phase = Phase::Result { winner, counts };
                info!("결과: P1 {} vs P2 {}", counts[0], counts[1]);
            }
        }
        Phase::Result { .. } => {}
    }
}

fn sync_tool(
    mut commands: Commands,
    battle: Res<Battle>,
    arsenal: Res<Arsenal>,
    mut tool: ResMut<PaintTool>,
    arsenal_buttons: Query<(Entity, &ArsenalButton)>,
    flip_h: Query<Entity, With<FlipHButton>>,
    flip_v: Query<Entity, With<FlipVButton>>,
) {
    if !battle.is_changed() {
        return;
    }
    let (enabled, player) = match battle.phase {
        Phase::Setup { player, .. } => (true, player),
        _ => (false, 0),
    };
    let mode = match arsenal.0.get(battle.arsenal) {
        Some((name, pattern)) if battle.arsenal > 0 => {
            let mut p = pattern.clone();
            if player == 1 {
                p = p.flip_h();
            }
            if battle.flip_h {
                p = p.flip_h();
            }
            if battle.flip_v {
                p = p.flip_v();
            }
            PaintMode::Stamp {
                name: name.clone(),
                pattern: p,
            }
        }
        _ => PaintMode::Pen,
    };
    tool.enabled = enabled;
    tool.player = player;
    tool.mode = mode;

    for (e, b) in &arsenal_buttons {
        if b.0 == battle.arsenal {
            commands.entity(e).insert(Selected);
        } else {
            commands.entity(e).remove::<Selected>();
        }
    }
    for e in &flip_h {
        if battle.flip_h {
            commands.entity(e).insert(Selected);
        } else {
            commands.entity(e).remove::<Selected>();
        }
    }
    for e in &flip_v {
        if battle.flip_v {
            commands.entity(e).insert(Selected);
        } else {
            commands.entity(e).remove::<Selected>();
        }
    }
}

fn update_territory(battle: Res<Battle>, mut tints: Query<(&TerritoryTint, &mut Sprite)>) {
    if !battle.is_changed() {
        return;
    }
    for (tint, mut sprite) in &mut tints {
        let base = if tint.0 == 0 { P1_COLOR } else { P2_COLOR };
        let alpha = match battle.phase {
            Phase::Setup { player, .. } if player == tint.0 => 0.16,
            Phase::Setup { .. } => 0.04,
            _ => 0.0,
        };
        let color = base.with_alpha(alpha);
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_hud(
    time: Res<Time>,
    generation: Res<Generation>,
    speed: Res<SimSpeed>,
    battle: Res<Battle>,
    mut hud: Query<(&Hud, &mut Text)>,
    mut bars: Query<(&Hud, &mut Node), Without<Text>>,
    mut ready_button: Query<&mut Node, (With<ReadyButton>, Without<Hud>, Without<FastButton>)>,
    mut fast_button: Query<&mut Node, (With<FastButton>, Without<Hud>, Without<ReadyButton>)>,
    mut result_panel: Query<&mut Visibility, With<ResultPanel>>,
) {
    let now = time.elapsed_secs_f64();
    let total = (battle.live[0] + battle.live[1]).max(1) as f32;

    for (kind, mut text) in &mut hud {
        let s = match kind {
            Hud::Phase => match battle.phase {
                Phase::Setup { player, .. } => format!("배치: {}", PLAYER_NAMES[player as usize]),
                Phase::Battle => "전투 중".into(),
                Phase::Finishing { .. } => "판정 중…".into(),
                Phase::Result { .. } => "경기 종료".into(),
            },
            Hud::Timer => match battle.phase {
                Phase::Setup { deadline, .. } => {
                    if deadline == f64::MAX {
                        "남은 시간 60초".into()
                    } else {
                        format!("남은 시간 {:.0}초", (deadline - now).max(0.0).ceil())
                    }
                }
                Phase::Battle | Phase::Finishing { .. } => format!(
                    "세대 {} / {}  ({:.0}배속)",
                    generation.0.min(BATTLE_GENS),
                    BATTLE_GENS,
                    speed.rate / BATTLE_RATE
                ),
                Phase::Result { .. } => format!("세대 {BATTLE_GENS} / {BATTLE_GENS}"),
            },
            Hud::Budget => match battle.phase {
                Phase::Setup { player, .. } => format!(
                    "예산 {} / {}셀",
                    BUDGET - battle.placed[player as usize],
                    BUDGET
                ),
                _ => String::new(),
            },
            Hud::Message => {
                if now < battle.message_until {
                    battle.message.clone()
                } else {
                    String::new()
                }
            }
            Hud::Score(p) => {
                let count = match battle.phase {
                    Phase::Setup { .. } => battle.placed[*p as usize],
                    _ => battle.live[*p as usize],
                };
                format!("{}: {}셀", PLAYER_NAMES[*p as usize], count)
            }
            Hud::ResultTitle => match battle.phase {
                Phase::Result {
                    winner: Some(w), ..
                } => format!("{} 승리!", PLAYER_NAMES[w as usize]),
                Phase::Result { winner: None, .. } => "무승부!".into(),
                _ => String::new(),
            },
            Hud::ResultDetail => match battle.phase {
                Phase::Result { counts, .. } => format!(
                    "{BATTLE_GENS}세대 후 살아남은 셀\n초록 {}  vs  주황 {}",
                    counts[0], counts[1]
                ),
                _ => String::new(),
            },
            Hud::ScoreBar(_) => continue,
        };
        set_text(&mut text, s);
    }

    for (kind, mut node) in &mut bars {
        if let Hud::ScoreBar(p) = kind {
            let ratio = match battle.phase {
                Phase::Setup { .. } => battle.placed[*p as usize] as f32 / BUDGET as f32,
                _ => battle.live[*p as usize] as f32 / total,
            };
            let width = percent((ratio * 100.0).clamp(0.0, 100.0));
            if node.width != width {
                node.width = width;
            }
        }
    }

    let in_setup = matches!(battle.phase, Phase::Setup { .. });
    let in_battle = matches!(battle.phase, Phase::Battle);
    let in_result = matches!(battle.phase, Phase::Result { .. });
    for mut node in &mut ready_button {
        let d = if in_setup {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != d {
            node.display = d;
        }
    }
    for mut node in &mut fast_button {
        let d = if in_battle {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != d {
            node.display = d;
        }
    }
    for mut visibility in &mut result_panel {
        let v = if in_result {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != v {
            *visibility = v;
        }
    }
}

fn tutorial_steps() -> Vec<TutorialStep> {
    vec![
        TutorialStep::new(
            "진영과 예산",
            "왼쪽 절반은 플레이어 1(초록), 오른쪽 절반은 플레이어 2(주황)의 진영입니다. 각자 60초 동안 60셀 예산으로 자기 진영에만 생명을 놓습니다.\n\
             상단에 남은 시간과 예산이 표시됩니다.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "직접 배치",
            "왼쪽 초록 진영 안에 셀 5개를 놓아 보세요. 오른쪽 클릭으로 되돌리면 예산이 돌아옵니다. 상대 진영에는 놓을 수 없습니다.",
            StepGoal::PaintCells(5),
        ),
        TutorialStep::new(
            "무기고 사용",
            "왼쪽 무기고에서 '글라이더'(숫자 키 2)를 고르고 초록 진영을 클릭해 찍어 보세요. H / V 키로 방향을 뒤집을 수 있습니다.\n\
             플레이어 2의 스탬프는 자동으로 좌우 반전되어 왼쪽을 향합니다.",
            StepGoal::Stamp(1),
        ),
        TutorialStep::new(
            "준비 완료",
            "배치가 끝나면 '준비 완료'(Enter)로 차례를 넘깁니다. 지금 눌러 보세요. 플레이어 2 차례가 되고, 한 번 더 누르면 전투가 시작됩니다.",
            StepGoal::Custom("ready"),
        ),
        TutorialStep::new(
            "전투와 색 다수결",
            "'준비 완료'를 한 번 더 눌러 전투를 시작하고, 30세대가 지나는 것을 지켜보세요.\n\
             새로 태어나는 셀은 부모 3개 중 더 많은 색을 따릅니다(초록 2 + 주황 1 → 초록). 왼쪽 점수 바가 실시간 셀 수입니다.",
            StepGoal::Generations(30),
        ),
        TutorialStep::new(
            "승패",
            "600세대가 지나면 살아남은 셀이 많은 쪽이 이깁니다. 'F'로 빨리 감기, 결과 후 'R'로 다시 시작합니다.\n\
             격자 가장자리는 반대편과 이어져 있으니 뒤에서 오는 글라이더도 조심하세요. 튜토리얼을 마치면 새 경기가 시작됩니다.",
            StepGoal::Info,
        ),
    ]
}
