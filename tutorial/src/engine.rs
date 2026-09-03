use bevy::{platform::collections::HashSet, prelude::*};
use conway_core::{
    camera::CameraActivity,
    grid::GridSize,
    paint::{PaintMode, PaintRequest, PaintSet, PaintTool},
    rle::{Pattern, parse_rle},
    sim::{Generation, GridSnapshot, ResetGrid, SimControl, SimSet},
    ui::{
        ACCENT, ButtonColors, INSET_BG, IntroDismissed, LINE, MUTED_COLOR, TEXT_COLOR, UiFont,
        UiSet, body_text, button, button_with, card, divider, hud_text, overlay_root, set_text,
        title_text,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepGoal {
    Info,
    PaintCells(u32),
    Stamp(u32),
    Generations(u64),
    Zoom,
    Shape(Pattern),
    Custom(&'static str),
}

impl StepGoal {
    fn needs_still_grid(&self) -> bool {
        matches!(self, Self::PaintCells(_) | Self::Stamp(_) | Self::Shape(_))
    }
}

fn shape_present(cells: &HashSet<IVec2>, shape: &Pattern) -> bool {
    let offsets: Vec<IVec2> = shape.alive_cells().map(|c| c.as_ivec2()).collect();
    let Some(first) = offsets.first().copied() else {
        return false;
    };
    let w = shape.width as i32;
    let h = shape.height as i32;
    cells.iter().any(|anchor| {
        let origin = *anchor - first;
        offsets.iter().all(|o| cells.contains(&(origin + *o)))
            && (-1..=w).all(|x| {
                (-1..=h).all(|y| {
                    let inside = x >= 0 && y >= 0 && x < w && y < h;
                    inside || !cells.contains(&(origin + IVec2::new(x, y)))
                })
            })
            && (0..w).all(|x| {
                (0..h).all(|y| {
                    shape.get(x as u32, y as u32) == cells.contains(&(origin + IVec2::new(x, y)))
                })
            })
    })
}

#[derive(Clone, Debug)]
pub struct TutorialStep {
    pub section: String,
    pub title: String,
    pub body: String,
    pub goal: StepGoal,
}

impl TutorialStep {
    pub fn new(title: impl Into<String>, body: impl Into<String>, goal: StepGoal) -> Self {
        Self {
            section: String::new(),
            title: title.into(),
            body: body.into(),
            goal,
        }
    }

    pub fn with_section(mut self, section: &str) -> Self {
        self.section = section.into();
        self
    }
}

fn shape(rle: &str) -> StepGoal {
    StepGoal::Shape(parse_rle(rle).expect("valid tutorial shape"))
}

pub fn rule_steps() -> Vec<TutorialStep> {
    vec![
        TutorialStep::new(
            "작은 생명들의 세상",
            "이 격자는 아주 작은 생명들이 사는 세상입니다. 칸 하나에 생명 하나가 살 수 있어요.\n\
             밝은 칸은 살아있는 생명, 어두운 칸은 빈 자리입니다.\n\
             시간은 '세대'라는 단위로 한 칸씩 흐르고, 세대가 바뀔 때마다 생명이 태어나거나 사라져요.\n\n\
             규칙은 딱 세 개. 외우지 말고 직접 해 보면서 알아봅시다. '다음'을 누르세요.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "생명 하나 놓기",
            "격자의 빈 곳을 한 번만 클릭해 보세요. 생명 하나가 생깁니다.",
            StepGoal::PaintCells(1),
        ),
        TutorialStep::new(
            "혼자는 외로워요",
            "아래 '한 세대' 버튼을 눌러 시간을 한 칸만 흘려 보세요. 방금 놓은 생명이 사라집니다.\n\n\
             규칙 ①  주변에 이웃이 없거나 하나뿐이면, 외로워서 사라져요.",
            StepGoal::Generations(1),
        ),
        TutorialStep::new(
            "셋이 나란히",
            "이번엔 빈 곳에 생명 셋을 가로로 딱 붙여 나란히 놓아 보세요. 주변 한 칸은 비워 두세요.",
            shape("x = 3, y = 1\n3o!"),
        ),
        TutorialStep::new(
            "새 생명이 태어나요",
            "'한 세대'를 눌러 보세요. 가로 셋이 세로 셋으로 바뀝니다!\n\
             양 끝은 이웃이 하나뿐이라 사라지고, 가운데는 이웃이 둘이라 살아남았어요.\n\
             가운데 위아래 빈칸은 주변에 생명이 딱 셋이라 새로 태어났고요.\n\n\
             규칙 ②  이웃이 둘이나 셋이면 살아남아요.\n\
             규칙 ③  빈칸 주변에 생명이 딱 셋이면 새로 태어나요.",
            StepGoal::Generations(1),
        ),
        TutorialStep::new(
            "깜빡이",
            "'재생 / 정지'를 눌러 시간을 계속 흘려 보세요. 가로와 세로를 번갈아 반복하죠?\n\
             이 모양의 이름은 '깜빡이'입니다. 생명 게임에서 가장 유명한 모양 중 하나예요.",
            StepGoal::Generations(10),
        ),
        TutorialStep::new(
            "네모 만들기",
            "이번에는 빈 곳에 생명 넷을 2×2 네모로 놓아 보세요.",
            shape("x = 2, y = 2\n2o$2o!"),
        ),
        TutorialStep::new(
            "변하지 않는 모양",
            "'한 세대'를 몇 번 눌러 보세요. 네모는 그대로죠?\n\
             네 생명 모두 이웃이 딱 셋이라 계속 살아남고, 주변 빈칸은 이웃이 둘 이하라 아무것도 태어나지 않아요.\n\
             이렇게 가만히 있는 모양을 '정물'이라고 부릅니다.",
            StepGoal::Generations(3),
        ),
        TutorialStep::new(
            "마음대로 낙서하기",
            "이제 격자를 드래그해서 아무렇게나 낙서해 보세요. 생명을 20개 넘게 놓으면 됩니다.",
            StepGoal::PaintCells(20),
        ),
        TutorialStep::new(
            "붐비면 사라져요",
            "'재생 / 정지'를 눌러 지켜보세요. 빽빽한 곳은 금방 비고, 셋이 모인 자리에선 새 생명이 태어나요.\n\
             잠시 뒤에는 깜빡이나 네모처럼 익숙한 모양만 남습니다.\n\n\
             규칙 ① (이어서)  이웃이 넷 이상이어도, 너무 붐벼서 사라져요.",
            StepGoal::Generations(40),
        ),
        TutorialStep::new(
            "규칙은 이게 전부예요",
            "살아있는 생명은 이웃이 둘이나 셋이면 살고, 그보다 적거나 많으면 사라져요.\n\
             빈칸은 주변에 생명이 딱 셋이면 새로 태어나요.\n\n\
             이 단순한 규칙에서 날아다니는 우주선, 시계, 심지어 컴퓨터까지 만들어집니다. 이제 조작법을 익혀 볼까요?",
            StepGoal::Info,
        ),
    ]
}

#[derive(Default, Clone, Debug)]
struct StepProgress {
    painted: u32,
    stamped: u32,
    generation_start: u64,
    zoomed: bool,
    custom: HashSet<&'static str>,
    done_at: Option<f64>,
    cells: HashSet<IVec2>,
}

#[derive(Resource, Clone, Debug)]
pub struct Tutorial {
    pub steps: Vec<TutorialStep>,
    pub active: bool,
    pub step: usize,
    progress: StepProgress,
    saved_control: Option<SimControl>,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct TutorialFinished;

impl Tutorial {
    pub fn new(steps: Vec<TutorialStep>) -> Self {
        Self {
            steps,
            active: false,
            step: 0,
            progress: StepProgress::default(),
            saved_control: None,
        }
    }

    pub fn start(&mut self) {
        self.active = true;
        self.step = 0;
        self.progress = StepProgress::default();
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn complete(&mut self, id: &'static str) {
        if self.active {
            self.progress.custom.insert(id);
        }
    }

    pub fn current(&self) -> Option<&TutorialStep> {
        self.active.then(|| self.steps.get(self.step)).flatten()
    }

    fn goal_status(&self, generation: u64) -> (bool, String) {
        let p = &self.progress;
        match self.current().map(|s| &s.goal) {
            Some(StepGoal::Info) => (true, String::new()),
            Some(StepGoal::PaintCells(n)) => {
                (p.painted >= *n, format!("{}/{n} 개", p.painted.min(*n)))
            }
            Some(StepGoal::Stamp(n)) => (p.stamped >= *n, format!("{}/{n} 개", p.stamped.min(*n))),
            Some(StepGoal::Generations(n)) => {
                let g = generation.saturating_sub(p.generation_start);
                (g >= *n, format!("{}/{n} 세대", g.min(*n)))
            }
            Some(StepGoal::Zoom) => (p.zoomed, String::new()),
            Some(StepGoal::Shape(shape)) => (shape_present(&p.cells, shape), String::new()),
            Some(StepGoal::Custom(id)) => (p.custom.contains(id), String::new()),
            None => (false, String::new()),
        }
    }

    fn advance(
        &mut self,
        generation: u64,
        control: &mut SimControl,
        activity: &mut CameraActivity,
    ) {
        self.step += 1;
        self.progress = StepProgress {
            generation_start: generation,
            cells: std::mem::take(&mut self.progress.cells),
            ..default()
        };
        if self.step >= self.steps.len() {
            self.active = false;
        }
        activity.zoomed = false;
        if self.current().is_some_and(|s| s.goal.needs_still_grid()) {
            control.paused = true;
            control.step_once = false;
        }
    }
}

#[derive(Component)]
struct TutorialOverlay;

#[derive(Component)]
struct StepStatus;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TutorialSet;

fn start_on_intro_dismissed(
    mut dismissed: MessageReader<IntroDismissed>,
    mut tutorial: ResMut<Tutorial>,
) {
    if dismissed.read().last().is_some() {
        tutorial.start();
    }
}

fn begin_tutorial(
    tutorial: &mut Tutorial,
    grid: &GridSize,
    control: &mut SimControl,
    tool: &mut PaintTool,
    generation: u64,
    reset: &mut MessageWriter<ResetGrid>,
) {
    tutorial.saved_control = Some(*control);
    tutorial.progress.generation_start = generation;
    control.paused = true;
    control.step_once = false;
    tool.enabled = true;
    tool.mode = PaintMode::Pen;
    reset.write(ResetGrid::empty(grid));
}

fn end_tutorial(
    tutorial: &mut Tutorial,
    grid: &GridSize,
    control: &mut SimControl,
    reset: &mut MessageWriter<ResetGrid>,
    finished: &mut MessageWriter<TutorialFinished>,
) {
    if let Some(saved) = tutorial.saved_control.take() {
        control.paused = saved.paused;
    }
    reset.write(ResetGrid::empty(grid));
    finished.write(TutorialFinished);
}

fn track_tutorial(
    time: Res<Time>,
    grid: Res<GridSize>,
    generation: Res<Generation>,
    mut activity: ResMut<CameraActivity>,
    mut tutorial: ResMut<Tutorial>,
    mut control: ResMut<SimControl>,
    mut tool: ResMut<PaintTool>,
    mut paints: MessageReader<PaintRequest>,
    snapshot: Option<Res<GridSnapshot>>,
    mut reset: MessageWriter<ResetGrid>,
    mut finished: MessageWriter<TutorialFinished>,
    mut was_active: Local<bool>,
) {
    let active = tutorial.active;
    if active && !*was_active {
        begin_tutorial(
            &mut tutorial,
            &grid,
            &mut control,
            &mut tool,
            generation.0,
            &mut reset,
        );
        activity.zoomed = false;
    } else if !active && *was_active {
        end_tutorial(
            &mut tutorial,
            &grid,
            &mut control,
            &mut reset,
            &mut finished,
        );
    }
    *was_active = active;
    if !active {
        paints.clear();
        return;
    }

    if let Some(snapshot) = snapshot.as_deref()
        && snapshot.is_ready()
    {
        let alive: HashSet<IVec2> = snapshot.alive_cells(&grid, 0).into_iter().collect();
        if alive != tutorial.progress.cells {
            tutorial.progress.cells = alive;
        }
    }

    let requests: Vec<PaintRequest> = paints.read().cloned().collect();
    let zoomed = activity.zoomed;
    if !requests.is_empty() || (zoomed && !tutorial.progress.zoomed) {
        let p = &mut tutorial.progress;
        p.zoomed |= zoomed;
        for req in &requests {
            if req.stamp {
                p.stamped += 1;
            }
            for cell in &req.cells {
                if req.alive {
                    if p.cells.insert(*cell) && !req.stamp {
                        p.painted += 1;
                    }
                } else {
                    p.cells.remove(cell);
                }
            }
        }
    }

    let (met, _) = tutorial.goal_status(generation.0);
    let is_info = matches!(tutorial.current().map(|s| &s.goal), Some(StepGoal::Info));
    let now = time.elapsed_secs_f64();
    match (met && !is_info, tutorial.progress.done_at) {
        (true, None) => tutorial.progress.done_at = Some(now),
        (true, Some(t)) if now - t >= 1.2 => {
            tutorial.advance(generation.0, &mut control, &mut activity);
        }
        _ => {}
    }
}

fn spawn_shape_preview(card: &mut ChildSpawnerCommands<'_>, font: &UiFont, shape: &Pattern) {
    const CELL: f32 = 16.0;
    const GAP: f32 = 2.0;
    card.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: px(12),
        ..default()
    })
    .with_children(|row| {
        row.spawn(hud_text(font, "이렇게:", 15.0, MUTED_COLOR));
        row.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(GAP),
                padding: UiRect::all(px(8)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(INSET_BG),
            BorderColor::all(LINE),
        ))
        .with_children(|grid| {
            for y in -1..=shape.height as i32 {
                grid.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(GAP),
                    ..default()
                })
                .with_children(|cells| {
                    for x in -1..=shape.width as i32 {
                        let inside =
                            x >= 0 && y >= 0 && x < shape.width as i32 && y < shape.height as i32;
                        let alive = inside && shape.get(x as u32, y as u32);
                        cells.spawn((
                            Node {
                                width: px(CELL),
                                height: px(CELL),
                                ..default()
                            },
                            BackgroundColor(if alive {
                                ACCENT
                            } else {
                                Color::srgb(0.13, 0.13, 0.14)
                            }),
                        ));
                    }
                });
            }
        });
    });
}

fn spawn_step_card(commands: &mut Commands, tutorial: &Tutorial, font: &UiFont, generation: u64) {
    let Some(step) = tutorial.current() else {
        return;
    };
    let (met, progress) = tutorial.goal_status(generation);
    let is_info = step.goal == StepGoal::Info;
    let show_sim_controls = matches!(step.goal, StepGoal::Generations(_));
    let total = tutorial.steps.len();
    let index = tutorial.step;
    let is_last = index + 1 == total;
    commands
        .spawn((overlay_root(false), TutorialOverlay))
        .with_children(|root| {
            root.spawn(card(760.0)).with_children(|card| {
                card.spawn(hud_text(
                    font,
                    format!("{}  ·  {}/{total}", step.section, index + 1),
                    14.0,
                    MUTED_COLOR,
                ));
                card.spawn(title_text(font, step.title.clone(), 23.0, TEXT_COLOR));
                card.spawn(body_text(font, step.body.clone(), 17.0));
                if let StepGoal::Shape(shape) = &step.goal {
                    spawn_shape_preview(card, font, shape);
                }
                card.spawn(divider());
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(Node {
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|left| {
                        if show_sim_controls {
                            left.spawn(button(font, "한 세대", 16.0)).observe(
                                |_: On<Pointer<Click>>, mut c: ResMut<SimControl>| {
                                    c.paused = true;
                                    c.step_once = true;
                                },
                            );
                            left.spawn(button(font, "재생 / 정지", 16.0)).observe(
                                |_: On<Pointer<Click>>, mut c: ResMut<SimControl>| {
                                    c.paused = !c.paused;
                                },
                            );
                        }
                        let status = if is_info {
                            String::new()
                        } else if met {
                            "완료! 잠시 후 다음 단계로 넘어갑니다".into()
                        } else if progress.is_empty() {
                            "진행 중…".into()
                        } else {
                            format!("진행 {progress}")
                        };
                        left.spawn((
                            hud_text(font, status, 16.0, if met { ACCENT } else { MUTED_COLOR }),
                            StepStatus,
                        ));
                    });
                    row.spawn(Node {
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|right| {
                        let label = match (is_info, is_last) {
                            (true, true) => "마치기",
                            (true, false) => "다음",
                            (false, _) => "건너뛰기",
                        };
                        right
                            .spawn(button_with(
                                font,
                                label,
                                16.0,
                                if is_info {
                                    ButtonColors::accent()
                                } else {
                                    ButtonColors::default()
                                },
                            ))
                            .observe(
                                |_: On<Pointer<Click>>,
                                 mut t: ResMut<Tutorial>,
                                 g: Res<Generation>,
                                 mut control: ResMut<SimControl>,
                                 mut activity: ResMut<CameraActivity>| {
                                    t.advance(g.0, &mut control, &mut activity);
                                },
                            );
                        if !is_last {
                            right
                                .spawn(button(font, "종료", 16.0))
                                .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.stop());
                        }
                    });
                });
            });
        });
}

fn sync_tutorial_overlay(
    mut commands: Commands,
    tutorial: Res<Tutorial>,
    generation: Res<Generation>,
    font: Res<UiFont>,
    existing: Query<Entity, With<TutorialOverlay>>,
    mut last: Local<Option<(bool, usize)>>,
) {
    let key = (tutorial.active, tutorial.step);
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    for e in &existing {
        commands.entity(e).despawn();
    }
    if tutorial.active {
        spawn_step_card(&mut commands, &tutorial, &font, generation.0);
    }
}

fn update_step_status(
    tutorial: Res<Tutorial>,
    generation: Res<Generation>,
    mut status: Query<(&mut Text, &mut TextColor), With<StepStatus>>,
) {
    let Some(step) = tutorial.current() else {
        return;
    };
    let (met, progress) = tutorial.goal_status(generation.0);
    let text = if step.goal == StepGoal::Info {
        String::new()
    } else if met {
        "완료! 잠시 후 다음 단계로 넘어갑니다".into()
    } else if progress.is_empty() {
        "진행 중…".into()
    } else {
        format!("진행 {progress}")
    };
    let color = if met { ACCENT } else { MUTED_COLOR };
    for (mut t, mut c) in &mut status {
        set_text(&mut t, text.as_str());
        if c.0 != color {
            c.0 = color;
        }
    }
}

pub struct TutorialPlugin {
    pub steps: Vec<TutorialStep>,
}

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tutorial::new(self.steps.clone()))
            .add_message::<TutorialFinished>()
            .add_systems(
                Update,
                (
                    start_on_intro_dismissed,
                    track_tutorial,
                    sync_tutorial_overlay,
                    update_step_status,
                )
                    .chain()
                    .in_set(TutorialSet)
                    .after(UiSet)
                    .after(PaintSet)
                    .after(SimSet),
            );
    }
}
