#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod engine;

use bevy::{prelude::*, window::PrimaryWindow};
use conway_core::{
    camera::{CameraControlPlugin, FitCameraPlugin, MainCamera, fit_camera},
    debug,
    grid::GridSize,
    paint::{PaintMode, PaintPlugin, PaintTool},
    rle::parse_rle,
    sim::{
        ConwaySimPlugin, Generation, GridView, InitialView, ResetGrid, SimConfig, SimControl,
        SimSet, SimSpeed,
    },
    ui::{
        ButtonColors, FestivalUiPlugin, Intro, MUTED_COLOR, Selected, SessionReset, TEXT_COLOR,
        TOP_RIGHT_RESERVE, UiFont, UiSet, button_with, hud_text, intro_closed, panel, set_text,
        title_text,
    },
};
use engine::{StepGoal, Tutorial, TutorialFinished, TutorialPlugin, TutorialSet, TutorialStep};

const GRID: UVec2 = UVec2::new(192, 108);
const TOP_BAR_H: f32 = 56.0;
const INITIAL_RATE: f32 = 10.0;
const RATES: &[f32] = &[1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0];

#[derive(Component)]
struct PenButton;

#[derive(Component)]
struct GliderButton;

#[derive(Component)]
struct PlayButton;

#[derive(Component, Clone, Copy)]
enum Hud {
    Generation,
    Speed,
}

#[derive(Message, Clone, Copy, Debug)]
enum Action {
    TogglePlay,
    Step,
    Clear,
    SpeedUp,
    SpeedDown,
    Fit,
    Pen,
    Glider,
}

fn main() -> AppExit {
    App::new()
        .add_plugins(conway_core::festival_default_plugins(
            "생명 게임 — 튜토리얼",
        ))
        .add_plugins((
            ConwaySimPlugin(SimConfig {
                size: GRID,
                initial_rate: INITIAL_RATE,
                start_paused: true,
                readback: true,
                initial_view: InitialView::FitGrid {
                    left_margin: 0.0,
                    top_margin: TOP_BAR_H,
                },
                ..default()
            }),
            CameraControlPlugin,
            PaintPlugin::default(),
            FitCameraPlugin {
                left_margin: 0.0,
                top_margin: TOP_BAR_H,
            },
            FestivalUiPlugin::new(
                "튜토리얼",
                "작은 생명들이 태어나고 사라지는 '생명 게임'을 직접 클릭하고 그려 보면서 배웁니다.\n\
                 처음이어도 괜찮아요. 화면 아래 안내를 따라 하나씩 해 보면 됩니다.",
            )
            .intro_hint("약 3~4분 걸립니다. 언제든 '종료'나 우상단 '처음으로'로 끝낼 수 있어요."),
            TutorialPlugin { steps: steps() },
            debug::ScreenshotPlugin {
                name: "tutorial".into(),
            },
        ))
        .add_message::<Action>()
        .add_systems(Startup, setup_ui)
        .add_systems(
            Update,
            (
                keyboard_shortcuts.run_if(intro_closed),
                handle_actions,
                on_session_reset,
                on_tutorial_finished,
                update_hud,
            )
                .chain()
                .after(SimSet)
                .after(UiSet)
                .after(TutorialSet),
        )
        .run()
}

fn steps() -> Vec<TutorialStep> {
    let rules = engine::rule_steps()
        .into_iter()
        .map(|s| s.with_section("1. 생명 게임 규칙"));
    let controls = vec![
        TutorialStep::new(
            "지우고 새로 시작",
            "격자가 어지러워졌죠? 상단 '지우기' 버튼(또는 C 키)을 눌러 깨끗하게 비워 보세요.\n\
             오른쪽 클릭으로 생명을 하나씩 지울 수도 있어요.",
            StepGoal::Custom("clear"),
        ),
        TutorialStep::new(
            "우주선 띄우기",
            "상단 '글라이더' 버튼(또는 G 키)을 누른 뒤 격자를 클릭하면, 그 자리에 '글라이더'라는 작은 우주선이 찍힙니다. 하나 찍어 보세요.",
            StepGoal::Stamp(1),
        ),
        TutorialStep::new(
            "날아가요!",
            "'재생 / 정지'를 눌러 보세요. 글라이더가 대각선으로 스르르 날아갑니다.\n\
             생명 다섯 개가 규칙 세 개만으로 움직이는 거예요. 화면 끝에 닿으면 반대쪽에서 다시 나옵니다.",
            StepGoal::Generations(20),
        ),
        TutorialStep::new(
            "빨리 감기",
            "상단 '속도 +' 버튼(또는 ] 키)을 눌러 시간을 더 빨리 흘려 보세요. '속도 -'( [ 키)로 다시 천천히.\n\
             재생 중이면 글라이더가 훨씬 빨리 날아가는 게 보일 거예요.",
            StepGoal::Custom("speed"),
        ),
        TutorialStep::new(
            "가까이 보기",
            "격자 위에서 마우스 휠을 굴려 확대하거나 축소해 보세요.\n\
             W A S D 키나 마우스 가운데 버튼 드래그로 움직이고, F 키를 누르면 다시 전체가 보입니다.",
            StepGoal::Zoom,
        ),
    ]
    .into_iter()
    .map(|s| s.with_section("2. 조작 익히기"));
    let modes = vec![
        TutorialStep::new(
            "자유 모드",
            "엄청나게 넓은 격자에 마음껏 그리고, 준비된 모양들을 불러와 구경하는 모드예요.\n\
             왼쪽 목록에서 모양을 클릭하면 화면 가운데에 놓입니다. 우주선, 글라이더를 쏘는 대포, 시계, 심지어 생명 게임 안에 만든 컴퓨터까지 있어요.\n\
             정해진 목표는 없으니 편하게 실험해 보세요.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "실시간 모양 맞추기",
            "왼쪽에 보이는 '목표 모양'을 격자 위에 최대한 빨리 만드는 게임이에요. 제한 시간은 60초.\n\
             단, '시작'을 누르면 시간이 계속 흐르면서 내가 놓은 생명도 규칙대로 변합니다. 방금 배운 대로 '어떤 모양이 그대로 남는지'를 떠올리면 유리해요.\n\
             똑같은 모양이 만들어지는 순간 자동으로 성공! 문제는 30분마다 바뀝니다.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "1 vs 1 대전",
            "두 사람이 대결하는 모드예요. 초록 편은 왼쪽, 주황 편은 오른쪽 진영에 각자 60초 동안 생명 60개를 놓습니다. 무기고에서 글라이더 같은 모양을 골라 찍을 수도 있어요.\n\
             둘 다 '준비 완료'를 누르면 600세대 동안 생존 경쟁! 새로 태어나는 생명은 주변에 더 많은 색을 따라가요.\n\
             마지막에 생명이 더 많이 남은 편이 이깁니다.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "준비 끝!",
            "수고했어요. 이제 부스의 세 모드 중 마음에 드는 것을 골라 즐겨 보세요.\n\
             '마치기'를 누르면 처음 화면으로 돌아갑니다.",
            StepGoal::Info,
        ),
    ]
    .into_iter()
    .map(|s| s.with_section("3. 축제의 세 가지 모드"));
    rules.chain(controls).chain(modes).collect()
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
            writer.write(action);
        },
    );
    ec
}

fn setup_ui(mut commands: Commands, font: Res<UiFont>) {
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
            padding: UiRect::new(px(14), px(TOP_RIGHT_RESERVE), px(8), px(8)),
            ..default()
        }))
        .with_children(|bar| {
            bar.spawn((
                title_text(&font, "튜토리얼", 21.0, TEXT_COLOR),
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
            action_button(
                bar,
                &font,
                "글라이더 (G)",
                ButtonColors::default(),
                Action::Glider,
            )
            .insert(GliderButton);
            bar.spawn((
                hud_text(&font, "", 16.0, TEXT_COLOR),
                Node {
                    margin: UiRect::left(px(14)),
                    ..default()
                },
                Hud::Generation,
            ));
            bar.spawn((hud_text(&font, "", 16.0, MUTED_COLOR), Hud::Speed));
        });
}

fn keyboard_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut writer: MessageWriter<Action>) {
    let map = [
        (KeyCode::Space, Action::TogglePlay),
        (KeyCode::KeyN, Action::Step),
        (KeyCode::KeyC, Action::Clear),
        (KeyCode::BracketRight, Action::SpeedUp),
        (KeyCode::BracketLeft, Action::SpeedDown),
        (KeyCode::KeyF, Action::Fit),
        (KeyCode::Home, Action::Fit),
        (KeyCode::KeyP, Action::Pen),
        (KeyCode::Escape, Action::Pen),
        (KeyCode::KeyG, Action::Glider),
    ];
    for (key, action) in map {
        if keys.just_pressed(key) {
            writer.write(action);
        }
    }
}

fn set_tool(
    commands: &mut Commands,
    tool: &mut PaintTool,
    mode: PaintMode,
    select: Entity,
    deselect: Entity,
) {
    tool.mode = mode;
    tool.enabled = true;
    commands.entity(select).insert(Selected);
    commands.entity(deselect).remove::<Selected>();
}

fn handle_actions(
    mut commands: Commands,
    mut reader: MessageReader<Action>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
    mut tool: ResMut<PaintTool>,
    mut tutorial: ResMut<Tutorial>,
    mut reset: MessageWriter<ResetGrid>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
    pen_button: Single<Entity, With<PenButton>>,
    glider_button: Single<Entity, With<GliderButton>>,
) {
    let (mut cam_tf, mut projection) = camera.into_inner();
    for action in reader.read() {
        match action {
            Action::TogglePlay => control.paused = !control.paused,
            Action::Step => {
                control.paused = true;
                control.step_once = true;
            }
            Action::Clear => {
                tutorial.complete("clear");
                reset.write(ResetGrid::empty(&grid));
            }
            Action::SpeedUp | Action::SpeedDown => {
                tutorial.complete("speed");
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
                speed.rate = RATES[new_idx];
            }
            Action::Fit => fit_camera(
                &mut cam_tf,
                &mut projection,
                &grid,
                &view,
                &window,
                0.0,
                TOP_BAR_H,
            ),
            Action::Pen => set_tool(
                &mut commands,
                &mut tool,
                PaintMode::Pen,
                *pen_button,
                *glider_button,
            ),
            Action::Glider => set_tool(
                &mut commands,
                &mut tool,
                PaintMode::Stamp {
                    name: "글라이더".into(),
                    pattern: parse_rle("x = 3, y = 3\nbob$2bo$3o!").expect("glider"),
                },
                *glider_button,
                *pen_button,
            ),
        }
    }
}

fn on_session_reset(
    mut resets: MessageReader<SessionReset>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    mut control: ResMut<SimControl>,
    mut speed: ResMut<SimSpeed>,
    mut tutorial: ResMut<Tutorial>,
    mut reset: MessageWriter<ResetGrid>,
    mut actions: MessageWriter<Action>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    if resets.read().last().is_none() {
        return;
    }
    tutorial.stop();
    reset.write(ResetGrid::empty(&grid));
    speed.rate = INITIAL_RATE;
    control.paused = true;
    control.step_once = false;
    actions.write(Action::Pen);
    let (mut tf, mut projection) = camera.into_inner();
    fit_camera(
        &mut tf,
        &mut projection,
        &grid,
        &view,
        &window,
        0.0,
        TOP_BAR_H,
    );
}

fn on_tutorial_finished(
    mut finished: MessageReader<TutorialFinished>,
    intro: Res<Intro>,
    mut session: MessageWriter<SessionReset>,
) {
    if finished.read().last().is_some() && !intro.open {
        session.write(SessionReset);
    }
}

fn update_hud(
    generation: Res<Generation>,
    speed: Res<SimSpeed>,
    control: Res<SimControl>,
    mut texts: Query<(&Hud, &mut Text)>,
    play_button: Single<&Children, With<PlayButton>>,
    mut labels: Query<&mut Text, Without<Hud>>,
) {
    for (hud, mut text) in &mut texts {
        match hud {
            Hud::Generation => set_text(&mut text, format!("세대 {}", generation.0)),
            Hud::Speed => set_text(&mut text, format!("{:.0} 세대/초", speed.rate)),
        }
    }
    for child in play_button.iter() {
        if let Ok(mut text) = labels.get_mut(child) {
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
