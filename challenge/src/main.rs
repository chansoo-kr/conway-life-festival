#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use conway_core::{
    camera::FitCameraPlugin,
    debug,
    grid::GridSize,
    paint::{PaintPlugin, PaintTool},
    presets::builtin_presets,
    qr,
    rle::Pattern,
    sim::{
        ConwaySimPlugin, Generation, GridColors, GridSnapshot, InitialView, ResetGrid, SimConfig,
        SimControl, SimSet, SimSpeed,
    },
    ui::{
        ACCENT, ButtonColors, FestivalUiPlugin, INSET_BG, LINE, MUTED_COLOR, SessionReset,
        TEXT_COLOR, UiFont, UiSet, button_with, hud_text, intro_closed, panel, set_text,
        title_text,
    },
};

const GRID: UVec2 = UVec2::new(64, 36);
const LEFT_PANEL_W: f32 = 380.0;
/// 라운드(목표 모양) 교체 주기. 웹 리더보드(`web/src/config.rs`)와 같아야
/// QR 안의 라운드 번호가 맞습니다.
const DEFAULT_INTERVAL_SECS: u64 = 60 * 60;
const MAX_BEST: usize = 5;
const DEFAULT_RATE: f32 = 1.0;
const RATES: &[f32] = &[0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0];
const DEFAULT_LIMIT_SECS: f32 = 60.0;
/// QR 모듈(점) 한 칸의 픽셀 수. 텍스처를 늘리지 않고 이 크기 그대로 띄웁니다.
const QR_MODULE_PX: u32 = 6;
/// 설명 문구를 QR 폭에 맞춰 접기 위한 대략적인 폭.
const QR_PX: f32 = 300.0;
/// 결과 QR을 찍고 이름을 적을 시간까지 감안한 결과 화면 유지 시간.
const RESULT_SECS: f64 = 25.0;
/// 스피드런 목표로 쓰는 프리셋 이름(앞부분). 실시간으로 진화하는 격자 위에서
/// 제한 시간 안에 만들 수 있어야 하므로 4x4 이하의 작은 모양만 넣습니다.
/// **순서를 바꾸거나 항목을 더하면 `web/src/round.rs` 의 `POOL` 도 똑같이 고쳐야
/// 합니다** (같은 라운드에 같은 모양이 나와야 QR 결과와 리더보드가 맞습니다).
/// 아래 테스트가 두 목록을 대조합니다.
const TARGET_POOL: &[&str] = &[
    "블록",
    "벌집",
    "빵",
    "보트",
    "튜브",
    "연못",
    "배",
    "긴 보트",
    "거룻배",
    "뱀",
    "항공모함",
    "깜빡이",
    "두꺼비",
    "비컨",
    "글라이더",
    "이터",
];

fn orientations(p: &Pattern) -> Vec<Pattern> {
    let mut out: Vec<Pattern> = Vec::new();
    let mut r = p.clone();
    for _ in 0..4 {
        for v in [r.clone(), r.flip_h()] {
            if let Some(n) = v.normalized()
                && !out.contains(&n)
            {
                out.push(n);
            }
        }
        r = r.rotate_cw();
    }
    out
}

#[derive(Clone, Debug)]
pub struct RoundInfo {
    pub id: u64,
    pub name: String,
    pub pattern: Pattern,
    pub variants: Vec<Pattern>,
    pub starts_at: u64,
    pub ends_at: u64,
}

pub trait ChallengeSource: Send + Sync + 'static {
    fn current_round(&self, now_unix: u64) -> RoundInfo;
}

pub struct LocalSource {
    pool: Vec<(String, Pattern)>,
    interval_secs: u64,
}

impl LocalSource {
    pub fn new(interval_secs: u64) -> Self {
        let presets = builtin_presets();
        let pool: Vec<(String, Pattern)> = TARGET_POOL
            .iter()
            .filter_map(|key| {
                let p = presets.iter().find(|p| p.name.starts_with(key))?;
                let n = p.pattern.normalized()?;
                Some((p.name.split(" (").next().unwrap_or(&p.name).to_string(), n))
            })
            .collect();
        assert!(!pool.is_empty(), "challenge pool is empty");
        Self {
            pool,
            interval_secs: interval_secs.max(10),
        }
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl ChallengeSource for LocalSource {
    fn current_round(&self, now_unix: u64) -> RoundInfo {
        let id = now_unix / self.interval_secs;
        let (name, pattern) = &self.pool[(splitmix64(id) % self.pool.len() as u64) as usize];
        RoundInfo {
            id,
            name: name.clone(),
            pattern: pattern.clone(),
            variants: orientations(pattern),
            starts_at: id * self.interval_secs,
            ends_at: (id + 1) * self.interval_secs,
        }
    }
}

#[derive(Resource)]
struct Source(Box<dyn ChallengeSource>);

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Idle,
    Playing {
        started: f64,
    },
    Cleared {
        time: f32,
        generation: u64,
        since: f64,
    },
    TimeUp {
        since: f64,
    },
}

#[derive(Resource)]
struct Challenge {
    round: RoundInfo,
    phase: Phase,
    best: Vec<f32>,
    rate: f32,
    limit: f32,
    accuracy: f32,
    best_accuracy: f32,
}

fn accuracy(drawn: &[IVec2], variants: &[Pattern]) -> f32 {
    if drawn.is_empty() || drawn.len() > 400 {
        return 0.0;
    }
    let drawn_set: std::collections::HashSet<IVec2> = drawn.iter().copied().collect();
    let stride = drawn.len().div_ceil(48).max(1);
    let mut best = 0.0f32;
    for target in variants {
        let cells: Vec<IVec2> = target.alive_cells().map(|c| c.as_ivec2()).collect();
        for d in drawn.iter().step_by(stride) {
            for t in &cells {
                let offset = *t - *d;
                let inter = cells
                    .iter()
                    .filter(|c| drawn_set.contains(&(**c - offset)))
                    .count();
                let union = cells.len() + drawn.len() - inter;
                best = best.max(inter as f32 / union.max(1) as f32);
            }
        }
    }
    best
}

#[derive(Component)]
struct PreviewBox;

/// 결과 QR 묶음(설명 + 코드). 평소에는 `Display::None`.
#[derive(Component)]
struct QrBox;

#[derive(Component)]
struct QrImage;

#[derive(Component)]
struct StartButton;

#[derive(Component, Clone, Copy)]
enum Hud {
    RoundName,
    Status,
    Timer,
    Sim,
    Best,
    Countdown,
}

#[derive(Message, Clone, Copy, Debug)]
enum Action {
    Start,
    Clear,
    RateUp,
    RateDown,
}

fn arg_value(name: &str) -> Option<String> {
    std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn main() -> AppExit {
    let interval = arg_value("--interval-min")
        .and_then(|v| v.parse::<u64>().ok())
        .map(|m| m * 60)
        .unwrap_or(DEFAULT_INTERVAL_SECS);
    let rate = arg_value("--rate")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|r| *r > 0.0)
        .unwrap_or(DEFAULT_RATE);
    let limit = arg_value("--limit-sec")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|s| *s > 0.0)
        .unwrap_or(DEFAULT_LIMIT_SECS);

    let source: Box<dyn ChallengeSource> = Box::new(LocalSource::new(interval));
    let round = source.current_round(unix_now());

    App::new()
        .add_plugins(conway_core::festival_default_plugins(
            "생명 게임 — 실시간 모양 맞추기",
        ))
        .add_plugins((
            ConwaySimPlugin(SimConfig {
                size: GRID,
                initial_rate: rate,
                start_paused: true,
                readback: true,
                initial_view: InitialView::FitGrid {
                    left_margin: LEFT_PANEL_W,
                    top_margin: 0.0,
                },
                colors: GridColors {
                    p1: LinearRgba::rgb(0.95, 0.80, 0.30),
                    ..default()
                },
                ..default()
            }),
            PaintPlugin::default(),
            FitCameraPlugin {
                left_margin: LEFT_PANEL_W,
                top_margin: 0.0,
            },
            FestivalUiPlugin::new(
                "실시간 모양 맞추기",
                "한 시간마다 바뀌는 목표 모양을, 실시간으로 계속 진화하는 격자 위에 최대한 빨리 만들어 내는 \
                 타임 어택 모드입니다. 내가 그린 셀도 규칙대로 변하니 진화를 피하거나 이용해야 합니다.\n\
                 끝나면 결과 QR이 뜹니다. 찍으면 리더보드에 이름을 올릴 수 있습니다.",
            ),
            debug::ScreenshotPlugin {
                name: "challenge".into(),
            },
        ))
        .insert_resource(Source(source))
        .insert_resource(Challenge {
            round,
            phase: Phase::Idle,
            best: Vec::new(),
            rate,
            limit,
            accuracy: 0.0,
            best_accuracy: 0.0,
        })
        .add_message::<Action>()
        .add_systems(Startup, setup_ui)
        .add_systems(
            Update,
            (
                poll_round,
                keyboard_shortcuts.run_if(intro_closed),
                handle_actions,
                check_match,
                tick_limit.run_if(intro_closed),
                on_session_reset,
                sync_tool.run_if(intro_closed),
                sync_qr,
                update_hud,
            )
                .chain()
                .after(SimSet)
                .after(UiSet),
        )
        .run()
}

fn setup_ui(mut commands: Commands, font: Res<UiFont>, challenge: Res<Challenge>) {
    let preview_pattern = challenge.round.pattern.clone();
    commands
        .spawn(panel(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            bottom: px(0),
            width: px(LEFT_PANEL_W),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(18)),
            row_gap: px(12),
            ..default()
        }))
        .with_children(|side| {
            side.spawn(title_text(&font, "실시간 모양 맞추기", 22.0, TEXT_COLOR));
            side.spawn((hud_text(&font, "", 17.0, TEXT_COLOR), Hud::RoundName));

            side.spawn(title_text(&font, "목표 모양", 15.0, MUTED_COLOR));
            let mut preview = side.spawn((
                PreviewBox,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(10)),
                    align_self: AlignSelf::FlexStart,
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(INSET_BG),
                BorderColor::all(LINE),
            ));
            preview.with_children(|p| build_preview(p, &preview_pattern));

            side.spawn((
                QrBox,
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    row_gap: px(8),
                    ..default()
                },
            ))
            .with_children(|qr_box| {
                qr_box.spawn(title_text(&font, "결과 QR", 15.0, MUTED_COLOR));
                qr_box.spawn((QrImage, ImageNode::default()));
                qr_box.spawn((
                    hud_text(&font, "찍으면 리더보드에 이름을 올릴 수 있습니다.", 14.0, MUTED_COLOR),
                    TextLayout::linebreak(LineBreak::WordOrCharacter),
                    Node {
                        max_width: px(QR_PX),
                        ..default()
                    },
                ));
            });

            side.spawn((hud_text(&font, "", 40.0, TEXT_COLOR), Hud::Timer));
            side.spawn((hud_text(&font, "", 16.0, ACCENT), Hud::Sim));
            side.spawn((
                hud_text(&font, "", 16.0, TEXT_COLOR),
                TextLayout::linebreak(LineBreak::WordOrCharacter),
                Hud::Status,
            ));

            side.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(8),
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(8),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    button_with(&font, "시작 (Space)", 18.0, ButtonColors::accent()),
                    StartButton,
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::Start);
                });
                row.spawn(button_with(
                    &font,
                    "지우기 (C)",
                    18.0,
                    ButtonColors::danger(),
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::Clear);
                });
                row.spawn(button_with(
                    &font,
                    "속도 - ([)",
                    15.0,
                    ButtonColors::default(),
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::RateDown);
                });
                row.spawn(button_with(
                    &font,
                    "속도 + (])",
                    15.0,
                    ButtonColors::default(),
                ))
                .observe(|_: On<Pointer<Click>>, mut w: MessageWriter<Action>| {
                    w.write(Action::RateUp);
                });
            });

            side.spawn((
                hud_text(&font, "", 16.0, TEXT_COLOR),
                Node {
                    margin: UiRect::top(px(10)),
                    ..default()
                },
                Hud::Best,
            ));

            side.spawn((
                hud_text(&font, "", 15.0, MUTED_COLOR),
                Node {
                    margin: UiRect::top(auto()),
                    ..default()
                },
                Hud::Countdown,
            ));
        });
}

fn build_preview(parent: &mut ChildSpawnerCommands<'_>, pattern: &Pattern) {
    let cell = (280.0 / pattern.width.max(pattern.height) as f32).clamp(8.0, 22.0);
    let gap = 1.0;
    for y in 0..pattern.height {
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(gap),
                margin: UiRect::bottom(px(gap)),
                ..default()
            })
            .with_children(|row| {
                for x in 0..pattern.width {
                    let alive = pattern.get(x, y);
                    row.spawn((
                        Node {
                            width: px(cell),
                            height: px(cell),
                            ..default()
                        },
                        BackgroundColor(if alive {
                            Color::srgb(0.95, 0.80, 0.30)
                        } else {
                            Color::srgb(0.13, 0.13, 0.14)
                        }),
                    ));
                }
            });
    }
}

fn poll_round(
    mut commands: Commands,
    source: Res<Source>,
    grid: Res<GridSize>,
    mut challenge: ResMut<Challenge>,
    mut control: ResMut<SimControl>,
    mut reset: MessageWriter<ResetGrid>,
    preview: Single<Entity, With<PreviewBox>>,
) {
    let now = unix_now();
    if now < challenge.round.ends_at {
        return;
    }
    let round = source.0.current_round(now);
    info!("new round #{}: {}", round.id, round.name);
    let pattern = round.pattern.clone();
    challenge.round = round;
    challenge.phase = Phase::Idle;
    challenge.best.clear();
    control.paused = true;
    reset.write(ResetGrid::empty(&grid));

    let entity = *preview;
    commands
        .entity(entity)
        .despawn_related::<Children>()
        .with_children(|p| build_preview(p, &pattern));
}

fn keyboard_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut writer: MessageWriter<Action>) {
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
        writer.write(Action::Start);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        writer.write(Action::Clear);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        writer.write(Action::RateDown);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        writer.write(Action::RateUp);
    }
}

fn handle_actions(
    mut reader: MessageReader<Action>,
    time: Res<Time>,
    grid: Res<GridSize>,
    mut challenge: ResMut<Challenge>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
    mut reset: MessageWriter<ResetGrid>,
    mut session: MessageWriter<SessionReset>,
) {
    for action in reader.read() {
        match action {
            Action::Start => {
                if matches!(
                    challenge.phase,
                    Phase::TimeUp { .. } | Phase::Cleared { .. }
                ) {
                    session.write(SessionReset);
                    continue;
                }
                challenge.accuracy = 0.0;
                challenge.best_accuracy = 0.0;
                reset.write(ResetGrid::empty(&grid));
                speed.rate = challenge.rate;
                control.paused = false;
                challenge.phase = Phase::Playing {
                    started: time.elapsed_secs_f64(),
                };
            }
            Action::Clear => {
                if matches!(challenge.phase, Phase::Playing { .. }) {
                    reset.write(ResetGrid::empty(&grid));
                }
            }
            Action::RateUp | Action::RateDown => {
                let idx = RATES
                    .iter()
                    .position(|r| (*r - challenge.rate).abs() < 0.01)
                    .unwrap_or_else(|| {
                        RATES
                            .iter()
                            .position(|r| *r > challenge.rate)
                            .unwrap_or(RATES.len() - 1)
                    });
                let new_idx = if matches!(action, Action::RateUp) {
                    (idx + 1).min(RATES.len() - 1)
                } else {
                    idx.saturating_sub(1)
                };
                challenge.rate = RATES[new_idx];
                if speed.rate != challenge.rate {
                    speed.rate = challenge.rate;
                }
            }
        }
    }
}

fn check_match(
    time: Res<Time>,
    grid: Res<GridSize>,
    generation: Res<Generation>,
    snapshot: Res<GridSnapshot>,
    mut challenge: ResMut<Challenge>,
    mut control: ResMut<SimControl>,
    mut reset: MessageWriter<ResetGrid>,
    mut last_seen: Local<u64>,
) {
    let Phase::Playing { started } = challenge.phase else {
        return;
    };
    if !snapshot.is_ready() || snapshot.received == *last_seen {
        return;
    }
    *last_seen = snapshot.received;

    let cells = snapshot.alive_cells(&grid, 0);
    let acc = accuracy(&cells, &challenge.round.variants);
    if (acc - challenge.accuracy).abs() > 0.001 {
        challenge.accuracy = acc;
        challenge.best_accuracy = challenge.best_accuracy.max(acc);
    }
    if cells.len() as u32 != challenge.round.pattern.alive_count() {
        return;
    }
    let Some(drawn) = Pattern::from_alive_cells(cells) else {
        return;
    };
    if !challenge.round.variants.contains(&drawn) {
        return;
    }

    control.paused = true;
    reset.write(ResetGrid(snapshot.words.clone()));
    let elapsed = (time.elapsed_secs_f64() - started) as f32;
    challenge.phase = Phase::Cleared {
        time: elapsed,
        generation: generation.0,
        since: time.elapsed_secs_f64(),
    };
    challenge.best.push(elapsed);
    challenge
        .best
        .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    challenge.best.truncate(MAX_BEST);
    info!("cleared in {elapsed:.2}s (generation {})", generation.0);
}

fn tick_limit(
    time: Res<Time>,
    grid: Res<GridSize>,
    mut challenge: ResMut<Challenge>,
    mut control: ResMut<SimControl>,
    mut reset: MessageWriter<ResetGrid>,
    mut session: MessageWriter<SessionReset>,
) {
    let now = time.elapsed_secs_f64();
    match challenge.phase {
        Phase::Playing { started } if now - started >= challenge.limit as f64 => {
            control.paused = true;
            reset.write(ResetGrid::empty(&grid));
            challenge.phase = Phase::TimeUp { since: now };
            info!(
                "time up after {:.0}s, best accuracy {:.0}%",
                challenge.limit,
                challenge.best_accuracy * 100.0
            );
        }
        Phase::TimeUp { since } if now - since >= RESULT_SECS => {
            session.write(SessionReset);
        }
        Phase::Cleared { since, .. } if now - since >= RESULT_SECS => {
            session.write(SessionReset);
        }
        _ => {}
    }
}

fn on_session_reset(
    mut resets: MessageReader<SessionReset>,
    grid: Res<GridSize>,
    mut challenge: ResMut<Challenge>,
    mut control: ResMut<SimControl>,
    mut reset: MessageWriter<ResetGrid>,
) {
    if resets.read().last().is_none() {
        return;
    }
    challenge.phase = Phase::Idle;
    challenge.accuracy = 0.0;
    challenge.best_accuracy = 0.0;
    control.paused = true;
    reset.write(ResetGrid::empty(&grid));
}

/// 결과가 나오면 그 결과를 담은 주소로 QR을 굽고, 목표 미리보기 자리에 대신 띄웁니다.
///
/// 주소에는 결과마다 다른 값이 들어가므로(중복 등록 방지) 매 프레임 새로 만들면 안 됩니다.
/// 결과가 바뀌었는지는 값이 아니라 아래 `key` 로만 판단합니다.
fn sync_qr(
    challenge: Res<Challenge>,
    mut images: ResMut<Assets<Image>>,
    mut qr_node: Query<&mut ImageNode, With<QrImage>>,
    mut boxes: Query<(&mut Node, Has<QrBox>), Or<(With<QrBox>, With<PreviewBox>)>>,
    mut shown: Local<Option<String>>,
) {
    let key = match challenge.phase {
        Phase::Cleared {
            time, generation, ..
        } => Some(format!("cleared {time} {generation}")),
        Phase::TimeUp { since } => Some(format!("timeup {since}")),
        _ => None,
    };
    if *shown == key {
        return;
    }
    if key.is_some() {
        let url = match challenge.phase {
            Phase::Cleared { time, .. } => {
                qr::challenge_url(challenge.round.id, true, 1.0, time)
            }
            _ => qr::challenge_url(
                challenge.round.id,
                false,
                challenge.best_accuracy,
                challenge.limit,
            ),
        };
        if let Some(image) = qr::qr_image(&url, QR_MODULE_PX, 2) {
            let handle = images.add(image);
            for mut node in &mut qr_node {
                node.image = handle.clone();
            }
        }
    }
    let showing_qr = key.is_some();
    for (mut node, is_qr) in &mut boxes {
        let want = if is_qr == showing_qr {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    *shown = key;
}

fn sync_tool(challenge: Res<Challenge>, mut tool: ResMut<PaintTool>) {
    let enabled = matches!(challenge.phase, Phase::Playing { .. });
    if tool.enabled != enabled {
        tool.enabled = enabled;
    }
}

fn update_hud(
    time: Res<Time>,
    generation: Res<Generation>,
    challenge: Res<Challenge>,
    mut hud: Query<(&Hud, &mut Text)>,
    start_button: Query<&Children, With<StartButton>>,
    mut texts: Query<&mut Text, Without<Hud>>,
) {
    let round = &challenge.round;
    let remaining = round.ends_at.saturating_sub(unix_now());
    for (kind, mut text) in &mut hud {
        let s = match kind {
            Hud::RoundName => format!(
                "라운드 #{}  —  {}  ({}x{}, {}셀)",
                round.id,
                round.name,
                round.pattern.width,
                round.pattern.height,
                round.pattern.alive_count()
            ),
            Hud::Status => match challenge.phase {
                Phase::Idle => "'시작'을 누르면 타이머와 시뮬레이션이 함께 출발합니다.\n\
                                격자는 계속 진화하니, 목표 모양이 나타나는 순간을 만드세요."
                    .into(),
                Phase::Playing { .. } => {
                    "진화 중… 격자 위 살아있는 셀 전체가 목표와 같아지면 판정됩니다.".into()
                }
                Phase::Cleared {
                    time, generation, ..
                } => format!(
                    "성공!  기록 {time:.2}초 (세대 {generation})\n왼쪽 QR을 찍으면 리더보드에 이름을 올릴 수 있습니다.\n잠시 후 처음 화면으로 돌아갑니다 (Space: 바로 돌아가기)."
                ),
                Phase::TimeUp { .. } => format!(
                    "시간 초과!  제한 시간 {:.0}초 안에 완성하지 못했습니다.\n최고 정확도 {:.0}%\n왼쪽 QR을 찍으면 리더보드에 이름을 올릴 수 있습니다.\n잠시 후 처음 화면으로 돌아갑니다 (Space: 바로 돌아가기).",
                    challenge.limit,
                    challenge.best_accuracy * 100.0
                ),
            },
            Hud::Timer => match challenge.phase {
                Phase::Idle => format!("{:.0} 초", challenge.limit),
                Phase::Playing { started } => format!(
                    "{:.1} 초 남음",
                    (challenge.limit as f64 - (time.elapsed_secs_f64() - started)).max(0.0)
                ),
                Phase::Cleared { time, .. } => format!("{time:.2} 초"),
                Phase::TimeUp { .. } => "시간 초과".into(),
            },
            Hud::Sim => match challenge.phase {
                Phase::Playing { .. } => format!(
                    "정확도 {:.0}% (최고 {:.0}%)  ·  세대 {}",
                    challenge.accuracy * 100.0,
                    challenge.best_accuracy * 100.0,
                    generation.0
                ),
                _ => format!(
                    "속도 {:.1} 세대/초  ·  세대 {}",
                    challenge.rate, generation.0
                ),
            },
            Hud::Best => {
                let mut s = String::from("이번 라운드 최고 기록");
                if challenge.best.is_empty() {
                    s.push_str("\n아직 없음");
                }
                for (i, t) in challenge.best.iter().enumerate() {
                    s.push_str(&format!("\n{}. {t:.2}초", i + 1));
                }
                s
            }
            Hud::Countdown => format!("다음 문제까지 {:02}:{:02}", remaining / 60, remaining % 60),
        };
        set_text(&mut text, s);
    }

    for children in &start_button {
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                set_text(
                    &mut text,
                    match challenge.phase {
                        Phase::Idle => "시작 (Space)",
                        Phase::Playing { .. } => "처음부터 (Space)",
                        Phase::Cleared { .. } | Phase::TimeUp { .. } => "처음으로 (Space)",
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `web/src/round.rs` 의 `POOL` 과 **순서·이름·모양이 모두** 같아야 합니다.
    /// 하나라도 어긋나면 같은 라운드에서 앱과 웹이 다른 모양을 가리킵니다.
    const WEB_POOL: &[(&str, &[&str])] = &[
        ("블록", &["oo", "oo"]),
        ("벌집", &[".oo.", "o..o", ".oo."]),
        ("빵", &[".oo.", "o..o", ".o.o", "..o."]),
        ("보트", &["oo.", "o.o", ".o."]),
        ("튜브", &[".o.", "o.o", ".o."]),
        ("연못", &[".oo.", "o..o", "o..o", ".oo."]),
        ("배", &["oo.", "o.o", ".oo"]),
        ("긴 보트", &[".o..", "o.o.", ".o.o", "..oo"]),
        ("거룻배", &[".o..", "o.o.", ".o.o", "..o."]),
        ("뱀", &["oo.o", "o.oo"]),
        ("항공모함", &["oo..", "o..o", "..oo"]),
        ("깜빡이", &["ooo"]),
        ("두꺼비", &[".ooo", "ooo."]),
        ("비컨", &["oo..", "oo..", "..oo", "..oo"]),
        ("글라이더", &[".o.", "..o", "ooo"]),
        ("이터", &["oo..", "o.o.", "..o.", "..oo"]),
    ];

    #[test]
    fn web_pool_matches_the_challenge_pool() {
        let source = LocalSource::new(DEFAULT_INTERVAL_SECS);
        assert_eq!(source.pool.len(), WEB_POOL.len(), "목록 길이");
        for (i, (name, pattern)) in source.pool.iter().enumerate() {
            let (web_name, web_shape) = WEB_POOL[i];
            assert_eq!(name, web_name, "{i}번째 이름");
            assert_eq!(pattern.height, web_shape.len() as u32, "{web_name} 높이");
            for (y, row) in web_shape.iter().enumerate() {
                assert_eq!(pattern.width, row.chars().count() as u32, "{web_name} 너비");
                for (x, c) in row.chars().enumerate() {
                    assert_eq!(
                        pattern.get(x as u32, y as u32),
                        c == 'o',
                        "{web_name} ({x},{y})"
                    );
                }
            }
        }
    }

    /// 주기가 같아야 QR 안의 라운드 번호가 웹의 라운드와 맞습니다.
    /// 여기를 바꾸면 `web/src/config.rs` 의 `ROUND_INTERVAL_SECS` 도 같이 바꿔야 합니다.
    #[test]
    fn the_round_interval_matches_the_web() {
        assert_eq!(DEFAULT_INTERVAL_SECS, 60 * 60);
    }

    /// 같은 시각이면 앱과 웹이 같은 라운드·같은 모양을 고릅니다.
    #[test]
    fn rounds_pick_the_same_shape_as_the_web() {
        let source = LocalSource::new(DEFAULT_INTERVAL_SECS);
        for now in [0u64, 1_756_000_000, 1_756_003_599, 1_756_003_600, 2_000_000_000] {
            let round = source.current_round(now);
            assert_eq!(round.id, now / 3600);
            // 웹과 같은 계산: POOL[splitmix64(id) % POOL.len()]
            let expected = WEB_POOL[(splitmix64(round.id) % WEB_POOL.len() as u64) as usize].0;
            assert_eq!(round.name, expected, "round {}", round.id);
        }
    }
}
