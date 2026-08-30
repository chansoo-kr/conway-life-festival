use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    platform::collections::HashSet,
    prelude::*,
    text::FontCx,
};

use crate::{
    camera::CameraActivity,
    grid::GridSize,
    paint::{PaintMode, PaintRequest, PaintSet, PaintTool},
    rle::{Pattern, parse_rle},
    sim::{Generation, ResetGrid, SimControl, SimSet},
};

#[derive(Resource, Clone)]
pub struct UiFont(pub FontSource);

const KOREAN_FAMILIES: &[&str] = &[
    "Pretendard",
    "Noto Sans KR",
    "Noto Sans CJK KR",
    "NanumGothic",
    "Nanum Gothic",
    "Malgun Gothic",
    "맑은 고딕",
    "Apple SD Gothic Neo",
    "Gulim",
    "굴림",
];

fn pick_ui_font(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut font_cx: ResMut<FontCx>,
) {
    if std::path::Path::new(&crate::asset_root())
        .join("fonts/ui.ttf")
        .exists()
    {
        info!("ui font: assets/fonts/ui.ttf");
        commands.insert_resource(UiFont(FontSource::Handle(
            asset_server.load("fonts/ui.ttf"),
        )));
        return;
    }

    let families: Vec<String> = font_cx
        .context
        .collection
        .family_names()
        .map(ToOwned::to_owned)
        .collect();
    for wanted in KOREAN_FAMILIES {
        if let Some(found) = families
            .iter()
            .find(|f| f.eq_ignore_ascii_case(wanted) || f.as_str() == *wanted)
        {
            info!("ui font: system family '{found}'");
            commands.insert_resource(UiFont(FontSource::Family(found.clone().into())));
            return;
        }
    }
    warn!("no Korean system font found; add assets/fonts/ui.ttf (falling back to sans-serif)");
    commands.insert_resource(UiFont(FontSource::SansSerif));
}

pub const PANEL_BG: Color = Color::srgba(0.09, 0.10, 0.13, 0.92);
pub const TEXT_COLOR: Color = Color::srgb(0.92, 0.93, 0.95);
pub const MUTED_COLOR: Color = Color::srgb(0.65, 0.68, 0.74);
pub const ACCENT: Color = Color::srgb(0.35, 0.85, 0.55);

#[derive(Component, Clone, Copy, Debug)]
pub struct ButtonColors {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
    pub selected: Color,
}

impl Default for ButtonColors {
    fn default() -> Self {
        Self {
            normal: Color::srgb(0.18, 0.20, 0.25),
            hovered: Color::srgb(0.26, 0.29, 0.36),
            pressed: Color::srgb(0.32, 0.55, 0.42),
            selected: Color::srgb(0.22, 0.48, 0.36),
        }
    }
}

impl ButtonColors {
    pub fn accent() -> Self {
        Self {
            normal: Color::srgb(0.20, 0.45, 0.32),
            hovered: Color::srgb(0.26, 0.58, 0.40),
            pressed: Color::srgb(0.35, 0.75, 0.50),
            selected: Color::srgb(0.30, 0.65, 0.45),
        }
    }
    pub fn danger() -> Self {
        Self {
            normal: Color::srgb(0.45, 0.20, 0.22),
            hovered: Color::srgb(0.58, 0.26, 0.28),
            pressed: Color::srgb(0.75, 0.35, 0.36),
            selected: Color::srgb(0.65, 0.30, 0.32),
        }
    }
}

#[derive(Component)]
pub struct Selected;

pub fn set_text(text: &mut Text, s: impl Into<String>) {
    let s = s.into();
    if text.0 != s {
        text.0 = s;
    }
}

pub fn hud_text(font: &UiFont, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.0.clone(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

pub fn button(font: &UiFont, label: impl Into<String>, size: f32) -> impl Bundle {
    button_with(font, label, size, ButtonColors::default())
}

pub fn button_with(
    font: &UiFont,
    label: impl Into<String>,
    size: f32,
    colors: ButtonColors,
) -> impl Bundle {
    button_styled(font, label, size, colors, Node::default())
}

pub fn button_styled(
    font: &UiFont,
    label: impl Into<String>,
    size: f32,
    colors: ButtonColors,
    mut node: Node,
) -> impl Bundle {
    if node.padding == UiRect::default() {
        node.padding = UiRect::axes(px(12), px(6));
    }
    if node.border == UiRect::default() {
        node.border = UiRect::all(px(1));
    }
    if node.border_radius == BorderRadius::default() {
        node.border_radius = BorderRadius::all(px(6));
    }
    if node.justify_content == JustifyContent::default() {
        node.justify_content = JustifyContent::Center;
    }
    if node.align_items == AlignItems::default() {
        node.align_items = AlignItems::Center;
    }
    (
        Button,
        node,
        BackgroundColor(colors.normal),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.12)),
        colors,
        children![hud_text(font, label, size, TEXT_COLOR)],
    )
}

pub fn panel(mut node: Node) -> impl Bundle {
    if node.border_radius == BorderRadius::default() {
        node.border_radius = BorderRadius::all(px(8));
    }
    (node, BackgroundColor(PANEL_BG), Interaction::None)
}

fn button_colors(
    mut buttons: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            Option<&ButtonColors>,
            Has<Selected>,
        ),
        With<Button>,
    >,
) {
    let default_colors = ButtonColors::default();
    for (interaction, mut bg, colors, selected) in &mut buttons {
        let c = colors.unwrap_or(&default_colors);
        let target = match (*interaction, selected) {
            (Interaction::Pressed, _) => c.pressed,
            (Interaction::Hovered, true) => c.selected.lighter(0.06),
            (Interaction::Hovered, false) => c.hovered,
            (Interaction::None, true) => c.selected,
            (Interaction::None, false) => c.normal,
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepGoal {
    Info,
    PaintCells(u32),
    EraseCells(u32),
    Stamp(u32),
    Generations(u64),
    Zoom,
    Shape(Pattern),
    Custom(&'static str),
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

    fn with_section(mut self, section: &str) -> Self {
        self.section = section.into();
        self
    }
}

pub fn common_rule_steps(paint_hint: &str) -> Vec<TutorialStep> {
    vec![
        TutorialStep::new(
            "격자와 셀",
            "격자의 칸 하나가 '셀'입니다. 셀은 살아있거나(밝은 색) 죽어 있고(어두운 색),\n\
             시간은 '세대' 단위로 흘러 매 세대 모든 셀이 동시에 바뀝니다.\n\
             셀의 운명은 주변 8칸(이웃) 중 몇 개가 살아있는지로만 정해집니다.\n\n\
             직접 해 보면서 익혀 봅시다. '다음'을 누르세요.",
            StepGoal::Info,
        ),
        TutorialStep::new(
            "셀 살리기",
            format!(
                "{paint_hint}격자를 왼쪽 클릭해서 셀 3개를 살려 보세요. 드래그하면 여러 칸을 한 번에 칠할 수 있습니다."
            ),
            StepGoal::PaintCells(3),
        ),
        TutorialStep::new(
            "셀 지우기",
            "이번에는 오른쪽 클릭으로 살아있는 셀 하나를 지워 보세요.",
            StepGoal::EraseCells(1),
        ),
        TutorialStep::new(
            "깜빡이 만들기",
            format!(
                "{paint_hint}빈 자리에 가로로 딱 붙여 나란히 셀 3개를 그려 보세요 (주변 한 칸은 비워 두세요). 이 모양의 이름은 '깜빡이'입니다."
            ),
            StepGoal::Shape(parse_rle("x = 3, y = 1\n3o!").expect("blinker")),
        ),
        TutorialStep::new(
            "한 세대 진행",
            "아래 '한 세대' 버튼을 눌러 시간을 한 칸 진행시켜 보세요.\n\
             양 끝 셀은 이웃이 1개뿐이라 죽고(고독), 가운데 셀의 위아래는 이웃이 정확히 3개라 새로 태어납니다.\n\
             그래서 가로 셋이 세로 셋으로 바뀝니다.",
            StepGoal::Generations(1),
        ),
        TutorialStep::new(
            "세 가지 규칙",
            "'재생'을 눌러 계속 진행시켜 보세요. 깜빡이가 2세대마다 같은 모양으로 돌아옵니다.\n\n\
             1. 살아있는 셀은 이웃이 2개 또는 3개면 살아남습니다.\n\
             2. 이웃이 1개 이하거나 4개 이상이면 죽습니다.\n\
             3. 죽은 셀은 이웃이 정확히 3개면 태어납니다.\n\
             이 셋이 규칙의 전부입니다.",
            StepGoal::Generations(10),
        ),
    ]
}

#[derive(Default, Clone, Debug)]
struct StepProgress {
    painted: u32,
    erased: u32,
    stamped: u32,
    generation_start: u64,
    zoomed: bool,
    custom: HashSet<&'static str>,
    done_at: Option<f64>,
    cells: HashSet<IVec2>,
}

#[derive(Resource, Clone, Debug)]
pub struct Tutorial {
    pub mode_name: String,
    pub mode_description: String,
    pub steps: Vec<TutorialStep>,
    pub intro_open: bool,
    pub active: bool,
    pub step: usize,
    progress: StepProgress,
    saved_control: Option<SimControl>,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct TutorialFinished;

impl Tutorial {
    pub fn new(
        mode_name: impl Into<String>,
        mode_description: impl Into<String>,
        steps: Vec<TutorialStep>,
    ) -> Self {
        Self {
            mode_name: mode_name.into(),
            mode_description: mode_description.into(),
            steps,
            intro_open: true,
            active: false,
            step: 0,
            progress: StepProgress::default(),
            saved_control: None,
        }
    }

    pub fn is_showing(&self) -> bool {
        self.intro_open || self.active
    }

    pub fn reset(&mut self) {
        self.intro_open = true;
        self.active = false;
        self.step = 0;
        self.progress = StepProgress::default();
        self.saved_control = None;
    }

    pub fn dismiss(&mut self) {
        self.intro_open = false;
        self.active = false;
    }

    pub fn start(&mut self) {
        self.intro_open = false;
        self.active = true;
        self.step = 0;
        self.progress = StepProgress::default();
    }

    pub fn toggle(&mut self) {
        if self.active {
            self.active = false;
        } else {
            self.start();
        }
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
                (p.painted >= *n, format!("{}/{n} 셀", p.painted.min(*n)))
            }
            Some(StepGoal::EraseCells(n)) => {
                (p.erased >= *n, format!("{}/{n} 셀", p.erased.min(*n)))
            }
            Some(StepGoal::Stamp(n)) => (p.stamped >= *n, format!("{}/{n} 회", p.stamped.min(*n))),
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

    fn advance(&mut self, generation: u64) -> bool {
        self.step += 1;
        self.progress = StepProgress {
            generation_start: generation,
            cells: std::mem::take(&mut self.progress.cells),
            ..default()
        };
        if self.step >= self.steps.len() {
            self.active = false;
            return true;
        }
        false
    }
}

pub fn intro_closed(tutorial: Res<Tutorial>) -> bool {
    !tutorial.intro_open
}

pub fn tutorial_inactive(tutorial: Res<Tutorial>) -> bool {
    !tutorial.is_showing()
}

#[derive(Component)]
struct TutorialOverlay;

#[derive(Component)]
struct TutorialButton;

#[derive(Message, Clone, Copy, Debug)]
pub struct SessionReset;

#[derive(Resource, Clone, Copy, Debug)]
pub struct SessionConfig {
    pub idle_reset_secs: f32,
}

pub fn idle_reset_from_args(default_secs: f32) -> f32 {
    std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == "--idle-reset-sec")
        .and_then(|w| w[1].parse::<f32>().ok())
        .unwrap_or(default_secs)
}

#[derive(Resource, Default)]
struct IdleClock {
    last_input: f64,
}

fn idle_watch(
    time: Res<Time>,
    config: Res<SessionConfig>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    tutorial: Res<Tutorial>,
    mut clock: ResMut<IdleClock>,
    mut reset: MessageWriter<SessionReset>,
) {
    let now = time.elapsed_secs_f64();
    let input = keys.get_just_pressed().next().is_some()
        || buttons.get_just_pressed().next().is_some()
        || motion.delta != Vec2::ZERO
        || scroll.delta != Vec2::ZERO;
    if input || tutorial.intro_open {
        clock.last_input = now;
        return;
    }
    if config.idle_reset_secs > 0.0 && now - clock.last_input > config.idle_reset_secs as f64 {
        clock.last_input = now;
        info!("idle for {:.0}s, resetting session", config.idle_reset_secs);
        reset.write(SessionReset);
    }
}

fn apply_session_reset(
    mut resets: MessageReader<SessionReset>,
    mut tutorial: ResMut<Tutorial>,
    mut activity: ResMut<CameraActivity>,
) {
    if resets.read().last().is_none() {
        return;
    }
    tutorial.reset();
    *activity = CameraActivity::default();
}

fn spawn_tutorial_button(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                right: px(12),
                column_gap: px(8),
                ..default()
            },
            GlobalZIndex(10),
        ))
        .with_children(|bar| {
            bar.spawn((
                button_with(&font, "튜토리얼 (T)", 18.0, ButtonColors::accent()),
                TutorialButton,
            ))
            .observe(|_: On<Pointer<Click>>, mut tutorial: ResMut<Tutorial>| {
                tutorial.toggle();
            });
            bar.spawn(button(&font, "처음으로", 18.0)).observe(
                |_: On<Pointer<Click>>, mut reset: MessageWriter<SessionReset>| {
                    reset.write(SessionReset);
                },
            );
        });
}

fn tutorial_keys(keys: Res<ButtonInput<KeyCode>>, mut tutorial: ResMut<Tutorial>) {
    if tutorial.intro_open {
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyY) {
            tutorial.start();
        } else if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyN) {
            tutorial.dismiss();
        }
        return;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        tutorial.toggle();
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
    activity: Res<CameraActivity>,
    mut tutorial: ResMut<Tutorial>,
    mut control: ResMut<SimControl>,
    mut tool: ResMut<PaintTool>,
    mut paints: MessageReader<PaintRequest>,
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

    let mut painted = 0u32;
    let mut erased = 0u32;
    let mut stamped = 0u32;
    let mut touched: Vec<(IVec2, bool)> = Vec::new();
    for req in paints.read() {
        if req.stamp {
            stamped += 1;
        } else if req.alive {
            painted += req.cells.len() as u32;
        } else {
            erased += req.cells.len() as u32;
        }
        touched.extend(req.cells.iter().map(|c| (*c, req.alive)));
    }
    let zoomed = activity.zoomed;
    if !touched.is_empty() || (zoomed && !tutorial.progress.zoomed) {
        let p = &mut tutorial.progress;
        p.painted += painted;
        p.erased += erased;
        p.stamped += stamped;
        p.zoomed |= zoomed;
        for (cell, alive) in touched {
            if alive {
                p.cells.insert(cell);
            } else {
                p.cells.remove(&cell);
            }
        }
    }

    let (met, _) = tutorial.goal_status(generation.0);
    let is_info = matches!(tutorial.current().map(|s| &s.goal), Some(StepGoal::Info));
    let now = time.elapsed_secs_f64();
    match (met && !is_info, tutorial.progress.done_at) {
        (true, None) => tutorial.progress.done_at = Some(now),
        (true, Some(t)) if now - t >= 1.2 && tutorial.advance(generation.0) => {
            end_tutorial(
                &mut tutorial,
                &grid,
                &mut control,
                &mut reset,
                &mut finished,
            );
            *was_active = false;
        }
        _ => {}
    }
}

fn overlay_root(modal: bool) -> impl Bundle {
    (
        TutorialOverlay,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: if modal {
                AlignItems::Center
            } else {
                AlignItems::FlexEnd
            },
            padding: UiRect::bottom(px(70)),
            ..default()
        },
        BackgroundColor(if modal {
            Color::srgba(0.0, 0.0, 0.0, 0.62)
        } else {
            Color::NONE
        }),
        Pickable {
            should_block_lower: modal,
            is_hoverable: modal,
        },
        GlobalZIndex(100),
    )
}

fn card(width: f32) -> impl Bundle {
    (
        Node {
            width: px(width),
            max_width: percent(92),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(24)),
            row_gap: px(12),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.11, 0.12, 0.16, 0.96)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15)),
        Interaction::None,
    )
}

fn body_text(font: &UiFont, s: String, size: f32) -> impl Bundle {
    (
        hud_text(font, s, size, TEXT_COLOR),
        TextLayout::linebreak(LineBreak::WordOrCharacter),
        Node {
            max_width: percent(100),
            ..default()
        },
    )
}

fn spawn_intro(commands: &mut Commands, tutorial: &Tutorial, font: &UiFont) {
    commands.spawn(overlay_root(true)).with_children(|root| {
        root.spawn(card(720.0)).with_children(|card| {
            card.spawn(hud_text(font, "생명 게임 축제", 15.0, MUTED_COLOR));
            card.spawn(hud_text(font, tutorial.mode_name.clone(), 32.0, ACCENT));
            card.spawn(body_text(font, tutorial.mode_description.clone(), 19.0));
            card.spawn(hud_text(
                font,
                "튜토리얼 모드를 하시겠습니까?\n직접 클릭하고 조작하면서 1. 생명 게임 규칙 → 2. 이 모드의 규칙 순서로 배웁니다.",
                19.0,
                TEXT_COLOR,
            ));
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                margin: UiRect::top(px(8)),
                ..default()
            })
            .with_children(|row| {
                row.spawn(button_with(
                    font,
                    "예, 튜토리얼 시작 (Enter)",
                    18.0,
                    ButtonColors::accent(),
                ))
                .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.start());
                row.spawn(button(font, "아니요, 바로 시작 (Esc)", 18.0))
                    .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.dismiss());
            });
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
    commands.spawn(overlay_root(false)).with_children(|root| {
        root.spawn(card(760.0)).with_children(|card| {
            card.spawn(hud_text(
                font,
                format!("튜토리얼  ·  {}  ({}/{total})", step.section, index + 1),
                14.0,
                MUTED_COLOR,
            ));
            card.spawn(hud_text(font, step.title.clone(), 24.0, ACCENT));
            card.spawn(body_text(font, step.body.clone(), 17.0));
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
                    left.spawn(hud_text(
                        font,
                        status,
                        16.0,
                        if met { ACCENT } else { MUTED_COLOR },
                    ));
                });
                row.spawn(Node {
                    column_gap: px(8),
                    ..default()
                })
                .with_children(|right| {
                    right
                        .spawn(button_with(
                            font,
                            if is_info { "다음" } else { "건너뛰기" },
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
                             grid: Res<GridSize>,
                             mut c: ResMut<SimControl>,
                             mut reset: MessageWriter<ResetGrid>,
                             mut finished: MessageWriter<TutorialFinished>| {
                                if t.advance(g.0) {
                                    end_tutorial(&mut t, &grid, &mut c, &mut reset, &mut finished);
                                }
                            },
                        );
                    right
                        .spawn(button(font, "종료", 16.0))
                        .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.active = false);
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
    mut last: Local<Option<(bool, bool, usize, String)>>,
) {
    let (met, progress) = tutorial.goal_status(generation.0);
    let key = (
        tutorial.intro_open,
        tutorial.active,
        tutorial.step,
        format!("{met}{progress}"),
    );
    if !tutorial.is_changed() && last.as_ref() == Some(&key) {
        return;
    }
    *last = Some(key);
    for e in &existing {
        commands.entity(e).despawn();
    }
    if tutorial.intro_open {
        spawn_intro(&mut commands, &tutorial, &font);
    } else if tutorial.active {
        spawn_step_card(&mut commands, &tutorial, &font, generation.0);
    }
}

pub struct FestivalUiPlugin {
    pub tutorial: Tutorial,
    pub idle_reset_secs: f32,
}

impl FestivalUiPlugin {
    pub fn new(
        mode_name: impl Into<String>,
        mode_description: impl Into<String>,
        mode_steps: Vec<TutorialStep>,
    ) -> Self {
        let mode_name = mode_name.into();
        let section = format!("2. {mode_name} 규칙");
        let steps = mode_steps
            .into_iter()
            .map(|s| s.with_section(&section))
            .collect();
        Self {
            tutorial: Tutorial::new(mode_name, mode_description, steps),
            idle_reset_secs: idle_reset_from_args(180.0),
        }
    }

    pub fn idle_reset(mut self, secs: f32) -> Self {
        self.idle_reset_secs = secs;
        self
    }

    pub fn with_rules(mut self, paint_hint: &str) -> Self {
        let mut steps: Vec<TutorialStep> = common_rule_steps(paint_hint)
            .into_iter()
            .map(|s| s.with_section("1. 생명 게임 규칙"))
            .collect();
        steps.append(&mut self.tutorial.steps);
        self.tutorial.steps = steps;
        self
    }
}

impl Plugin for FestivalUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.tutorial.clone())
            .insert_resource(SessionConfig {
                idle_reset_secs: self.idle_reset_secs,
            })
            .init_resource::<IdleClock>()
            .init_resource::<CameraActivity>()
            .add_message::<TutorialFinished>()
            .add_message::<SessionReset>()
            .add_systems(PreStartup, pick_ui_font)
            .add_systems(Startup, spawn_tutorial_button)
            .add_systems(
                Update,
                (
                    button_colors,
                    idle_watch,
                    apply_session_reset,
                    tutorial_keys,
                    track_tutorial.after(PaintSet).after(SimSet),
                    sync_tutorial_overlay,
                )
                    .chain(),
            );
    }
}
