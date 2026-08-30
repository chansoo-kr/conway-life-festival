#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use conway_core::{
    camera::FitCameraPlugin,
    debug,
    grid::GridSize,
    paint::{PaintPlugin, PaintTool},
    presets::builtin_presets,
    rle::Pattern,
    sim::{
        ConwaySimPlugin, Generation, GridColors, GridSnapshot, InitialView, ResetGrid, SimConfig,
        SimControl, SimSet, SimSpeed,
    },
    ui::{
        ACCENT, ButtonColors, FestivalUiPlugin, MUTED_COLOR, StepGoal, TEXT_COLOR, Tutorial,
        TutorialFinished, TutorialStep, UiFont, button_with, hud_text, intro_closed, panel,
        set_text, tutorial_inactive,
    },
};

const GRID: UVec2 = UVec2::new(64, 36);
const LEFT_PANEL_W: f32 = 380.0;
const DEFAULT_INTERVAL_SECS: u64 = 30 * 60;
const MAX_BEST: usize = 5;
const DEFAULT_RATE: f32 = 1.0;
const RATES: &[f32] = &[0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0];
const DEFAULT_LIMIT_SECS: f32 = 60.0;
const RESULT_SECS: f64 = 8.0;
const TARGET_POOL: &[&str] = &[
    "블록",
    "벌집",
    "빵",
    "보트",
    "튜브",
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
    Playing { started: f64 },
    Cleared { time: f32, generation: u64 },
    TimeUp { since: f64 },
}

#[derive(Resource)]
struct Challenge {
    round: RoundInfo,
    phase: Phase,
    best: Vec<f32>,
    rate: f32,
    limit: f32,
}

#[derive(Component)]
struct PreviewBox;

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
                "30분마다 바뀌는 목표 모양을, 실시간으로 계속 진화하는 격자 위에 최대한 빨리 만들어 내는 \
                 타임 어택 모드입니다. 내가 그린 셀도 규칙대로 변하니 진화를 피하거나 이용해야 합니다.",
                tutorial_steps(),
            )
            .with_rules(""),
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
        })
        .add_message::<Action>()
        .add_systems(Startup, setup_ui)
        .add_systems(
            Update,
            (
                poll_round,
                keyboard_shortcuts.run_if(intro_closed),
                handle_actions,
                on_tutorial_finished,
                check_match,
                tick_limit,
                sync_tool.run_if(tutorial_inactive),
                update_hud,
            )
                .chain()
                .after(SimSet),
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
            side.spawn(hud_text(&font, "실시간 모양 맞추기", 24.0, ACCENT));
            side.spawn((hud_text(&font, "", 17.0, TEXT_COLOR), Hud::RoundName));

            side.spawn(hud_text(&font, "목표 모양", 15.0, MUTED_COLOR));
            let mut preview = side.spawn((
                PreviewBox,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(10)),
                    align_self: AlignSelf::FlexStart,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.06, 0.07, 0.09)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15)),
            ));
            preview.with_children(|p| build_preview(p, &preview_pattern));

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
                            Color::srgb(0.12, 0.13, 0.16)
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
    mut tutorial: ResMut<Tutorial>,
) {
    for action in reader.read() {
        match action {
            Action::Start => {
                tutorial.complete("start");
                if matches!(challenge.phase, Phase::TimeUp { .. }) {
                    challenge.phase = Phase::Idle;
                    continue;
                }
                reset.write(ResetGrid::empty(&grid));
                speed.rate = challenge.rate;
                control.paused = false;
                challenge.phase = Phase::Playing {
                    started: time.elapsed_secs_f64(),
                };
            }
            Action::Clear => {
                tutorial.complete("clear");
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
) {
    let now = time.elapsed_secs_f64();
    match challenge.phase {
        Phase::Playing { started } if now - started >= challenge.limit as f64 => {
            control.paused = true;
            reset.write(ResetGrid::empty(&grid));
            challenge.phase = Phase::TimeUp { since: now };
            info!("time up after {:.0}s", challenge.limit);
        }
        Phase::TimeUp { since } if now - since >= RESULT_SECS => {
            challenge.phase = Phase::Idle;
        }
        _ => {}
    }
}

fn on_tutorial_finished(
    mut finished: MessageReader<TutorialFinished>,
    grid: Res<GridSize>,
    mut challenge: ResMut<Challenge>,
    mut control: ResMut<SimControl>,
    mut reset: MessageWriter<ResetGrid>,
) {
    if finished.read().last().is_none() {
        return;
    }
    challenge.phase = Phase::Idle;
    control.paused = true;
    reset.write(ResetGrid::empty(&grid));
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
                Phase::Cleared { time, generation } => format!(
                    "성공!  기록 {time:.2}초 (세대 {generation})\n'다시 도전'으로 기록을 갱신해 보세요."
                ),
                Phase::TimeUp { .. } => format!(
                    "시간 초과!  제한 시간 {:.0}초 안에 완성하지 못했습니다.\n잠시 후 처음 화면으로 돌아갑니다 (Space: 바로 돌아가기).",
                    challenge.limit
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
            Hud::Sim => format!(
                "속도 {:.1} 세대/초  ·  세대 {}",
                challenge.rate, generation.0
            ),
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
                        Phase::Cleared { .. } => "다시 도전 (Space)",
                        Phase::TimeUp { .. } => "처음으로 (Space)",
                    },
                );
            }
        }
    }
}

fn tutorial_steps() -> Vec<TutorialStep> {
    vec![
        TutorialStep::new(
            "목표 모양 확인",
            "왼쪽 패널의 '목표 모양'이 이번 라운드의 문제입니다. 격자 어디든 이 모양과 똑같은 모양(살아있는 셀 전체)을 만들면 성공입니다. 돌리거나 뒤집은 모양도 인정됩니다.\n\
             한 번 도전할 때 제한 시간이 있고(기본 60초), 시간이 다 되면 결과를 보여 준 뒤 처음 화면으로 돌아갑니다.\n\
             문제는 30분마다 바뀌고, 패널 아래에 다음 문제까지 남은 시간이 표시됩니다.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "라운드 시작",
            "'시작' 버튼(또는 Space)을 눌러 보세요. 타이머와 함께 시뮬레이션이 실시간으로 돌아가기 시작합니다.",
            StepGoal::Custom("start"),
        ),
        TutorialStep::new(
            "진화하는 격자에 그리기",
            "격자에 셀을 5개 그려 보세요. 그린 셀이 규칙대로 태어나고 죽는 것을 보세요.\n\
             혼자 떨어진 셀은 다음 세대에 사라지고, 셀 3개가 일렬이면 깜빡이가 됩니다. 패널의 '세대' 숫자가 오르는 리듬을 눈여겨 두세요.",
            StepGoal::PaintCells(5),
        ),
        TutorialStep::new(
            "지우고 다시",
            "엉망이 됐으면 '지우기'(C)로 격자를 비울 수 있습니다. 타이머는 계속 갑니다. 한 번 눌러 보세요.",
            StepGoal::Custom("clear"),
        ),
        TutorialStep::new(
            "판정과 기록",
            "격자 위 살아있는 셀 전체가 목표와 같아지는 순간 자동으로 판정되어 기록이 남고, 그 순간의 격자가 고정됩니다.\n\
             정지 패턴은 완성 순서를, 움직이는 패턴은 완성되는 순간을 노리세요. 제한 시간 안에 못 만들면 처음으로 돌아갑니다.\n\
             튜토리얼을 마치면 라운드가 처음 상태로 돌아갑니다. 행운을 빕니다!",
            StepGoal::Info,
        ),
    ]
}
