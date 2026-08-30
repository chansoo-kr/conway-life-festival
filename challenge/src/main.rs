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
        ACCENT, ButtonColors, FestivalUiPlugin, MUTED_COLOR, TEXT_COLOR, TutorialPage, UiFont,
        button_with, hud_text, panel, set_text, tutorial_closed,
    },
};

const GRID: UVec2 = UVec2::new(64, 36);
const LEFT_PANEL_W: f32 = 380.0;
const DEFAULT_INTERVAL_SECS: u64 = 30 * 60;
const MAX_BEST: usize = 5;
const DEFAULT_RATE: f32 = 1.0;
const RATES: &[f32] = &[0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0];
const MAX_TARGET_SIZE: u32 = 13;

#[derive(Clone, Debug)]
pub struct RoundInfo {
    pub id: u64,
    pub name: String,
    pub pattern: Pattern,
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
        let pool: Vec<(String, Pattern)> = builtin_presets()
            .into_iter()
            .filter_map(|p| {
                let n = p.pattern.normalized()?;
                (n.width <= MAX_TARGET_SIZE && n.height <= MAX_TARGET_SIZE && n.alive_count() >= 4)
                    .then(|| (p.name.split(" (").next().unwrap_or(&p.name).to_string(), n))
            })
            .collect();
        assert!(!pool.is_empty(), "문제 풀이 비어 있습니다");
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
}

#[derive(Resource)]
struct Challenge {
    round: RoundInfo,
    phase: Phase,
    best: Vec<f32>,
    rate: f32,
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
            FestivalUiPlugin::new(tutorial_pages()).open_at_start(true),
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
        })
        .add_message::<Action>()
        .add_systems(Startup, setup_ui)
        .add_systems(
            Update,
            (
                poll_round,
                keyboard_shortcuts.run_if(tutorial_closed),
                handle_actions,
                check_match,
                sync_tool,
                update_hud,
            )
                .chain()
                .after(SimSet),
        )
        .run()
}

fn tutorial_pages() -> Vec<TutorialPage> {
    vec![
        TutorialPage::new(
            "실시간 모양 맞추기",
            "왼쪽의 '목표 모양'을 오른쪽 격자 위에 똑같이 만들어 내는 게임입니다. 위치는 상관없습니다.\n\n\
             단, 격자는 멈춰 있지 않습니다! '시작'을 누르는 순간부터 생명 게임 규칙이 실시간으로 돌아가\n\
             내가 그린 셀도 계속 태어나고 죽습니다. 그 흐름을 피하거나 이용해서\n\
             목표와 같은 모양이 격자에 나타나는 순간을 만드세요. 그 순간 자동으로 판정됩니다.",
        ),
        TutorialPage::new(
            "생명 게임 규칙",
            "매 세대마다 모든 셀이 동시에 바뀝니다.\n\
             • 살아있는 셀: 이웃(주변 8칸)이 2개 또는 3개면 살아남고, 아니면 죽습니다.\n\
             • 죽은 셀: 이웃이 정확히 3개면 새로 태어납니다.\n\n\
             혼자 떨어진 셀 1~2개는 다음 세대에 사라지고, 셀 3개가 일렬이면 깜빡이가 됩니다.\n\
             블록·벌집 같은 '정지 패턴'은 완성만 하면 그대로 유지되지만, 완성 직전의 반쪽짜리 모양이\n\
             먼저 무너질 수 있으니 순서를 궁리해야 합니다.",
        ),
        TutorialPage::new(
            "진행 방법",
            "1. '시작'(Space)을 누르면 타이머와 시뮬레이션이 함께 출발합니다.\n\
             2. 왼쪽 클릭/드래그로 셀을 살리고, 오른쪽 클릭으로 지웁니다. 드래그가 빠릅니다!\n\
             3. 엉망이 됐으면 '지우기'(C)로 전부 비우고 다시 그립니다 (타이머는 계속 갑니다).\n\
             4. 격자 위 살아있는 셀 전체가 목표와 같아지는 순간 '성공!'과 함께 기록이 남습니다.\n\n\
             패널의 '세대' 숫자가 오르는 리듬을 보고, 다음 세대 직후에 마지막 셀을 넣는 것이 요령입니다.",
        ),
        TutorialPage::new(
            "라운드와 난이도",
            "문제는 30분마다 바뀝니다. 패널 아래에 다음 문제까지 남은 시간이 보입니다.\n\
             이번 라운드 최고 기록 5개가 왼쪽에 남고, '다시 도전'으로 몇 번이고 기록에 도전할 수 있습니다.\n\n\
             진행 요원용: [ 와 ] 키로 시뮬레이션 속도(세대/초)를 바꿔 난이도를 조절할 수 있습니다.",
        ),
    ]
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
    info!("새 라운드 #{}: {}", round.id, round.name);
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
) {
    for action in reader.read() {
        match action {
            Action::Start => {
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
    if cells.len() as u32 != challenge.round.pattern.alive_count() {
        return;
    }
    let Some(drawn) = Pattern::from_alive_cells(cells) else {
        return;
    };
    if drawn != challenge.round.pattern {
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
    info!("클리어! {elapsed:.2}초 (세대 {})", generation.0);
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
            },
            Hud::Timer => match challenge.phase {
                Phase::Idle => "0.00 초".into(),
                Phase::Playing { started } => {
                    format!("{:.2} 초", time.elapsed_secs_f64() - started)
                }
                Phase::Cleared { time, .. } => format!("{time:.2} 초"),
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
                    },
                );
            }
        }
    }
}
