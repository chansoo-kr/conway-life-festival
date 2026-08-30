use bevy::{prelude::*, text::FontCx};

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
        info!("UI 폰트: assets/fonts/ui.ttf");
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
            info!("UI 폰트: 시스템 패밀리 '{found}'");
            commands.insert_resource(UiFont(FontSource::Family(found.clone().into())));
            return;
        }
    }
    warn!(
        "한글 시스템 폰트를 찾지 못했습니다. assets/fonts/ui.ttf 를 넣어 주세요. (Sans-serif 대체)"
    );
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

#[derive(Clone, Debug)]
pub struct TutorialPage {
    pub title: String,
    pub body: String,
}

impl TutorialPage {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Tutorial {
    pub pages: Vec<TutorialPage>,
    pub open: bool,
    pub page: usize,
    pub open_at_start: bool,
}

impl Tutorial {
    pub fn new(pages: Vec<TutorialPage>) -> Self {
        Self {
            pages,
            open: false,
            page: 0,
            open_at_start: false,
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.page = 0;
        }
    }

    pub fn next(&mut self) {
        if self.page + 1 < self.pages.len() {
            self.page += 1;
        } else {
            self.open = false;
        }
    }

    pub fn prev(&mut self) {
        self.page = self.page.saturating_sub(1);
    }
}

pub fn tutorial_closed(tutorial: Res<Tutorial>) -> bool {
    !tutorial.open
}

#[derive(Component)]
struct TutorialOverlay;

fn spawn_tutorial_button(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                right: px(12),
                ..default()
            },
            GlobalZIndex(10),
            children![(
                button_with(&font, "튜토리얼 (T)", 18.0, ButtonColors::accent()),
                TutorialButton,
            )],
        ))
        .observe(|_: On<Pointer<Click>>, mut tutorial: ResMut<Tutorial>| {
            tutorial.toggle();
        });
}

#[derive(Component)]
struct TutorialButton;

fn tutorial_keys(keys: Res<ButtonInput<KeyCode>>, mut tutorial: ResMut<Tutorial>) {
    if keys.just_pressed(KeyCode::KeyT) {
        tutorial.toggle();
    }
    if !tutorial.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        tutorial.open = false;
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::Space) {
        tutorial.next();
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        tutorial.prev();
    }
}

fn open_at_start(mut tutorial: ResMut<Tutorial>) {
    if tutorial.open_at_start {
        tutorial.open = true;
        tutorial.page = 0;
    }
}

fn sync_tutorial_overlay(
    mut commands: Commands,
    tutorial: Res<Tutorial>,
    font: Res<UiFont>,
    existing: Query<Entity, With<TutorialOverlay>>,
) {
    if !tutorial.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !tutorial.open || tutorial.pages.is_empty() {
        return;
    }
    let page = tutorial.page.min(tutorial.pages.len() - 1);
    let p = &tutorial.pages[page];
    let is_last = page + 1 >= tutorial.pages.len();

    commands
        .spawn((
            TutorialOverlay,
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            Interaction::None,
            GlobalZIndex(100),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(720),
                    max_width: percent(92),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(28)),
                    row_gap: px(16),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.11, 0.12, 0.16)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                Interaction::None,
            ))
            .with_children(|card| {
                card.spawn(hud_text(
                    &font,
                    format!("튜토리얼  {}/{}", page + 1, tutorial.pages.len()),
                    15.0,
                    MUTED_COLOR,
                ));
                card.spawn(hud_text(&font, p.title.clone(), 30.0, ACCENT));
                card.spawn((
                    hud_text(&font, p.body.clone(), 19.0, TEXT_COLOR),
                    TextLayout::linebreak(LineBreak::WordOrCharacter),
                    Node {
                        max_width: percent(100),
                        ..default()
                    },
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    margin: UiRect::top(px(8)),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(Node {
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|left| {
                        if page > 0 {
                            left.spawn(button(&font, "이전", 18.0))
                                .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.prev());
                        }
                        left.spawn(button_with(
                            &font,
                            if is_last { "완료" } else { "다음" },
                            18.0,
                            ButtonColors::accent(),
                        ))
                        .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.next());
                    });
                    row.spawn(button(&font, "닫기 (Esc)", 18.0))
                        .observe(|_: On<Pointer<Click>>, mut t: ResMut<Tutorial>| t.open = false);
                });
            });
        });
}

pub struct FestivalUiPlugin {
    pub tutorial: Tutorial,
}

impl FestivalUiPlugin {
    pub fn new(pages: Vec<TutorialPage>) -> Self {
        Self {
            tutorial: Tutorial::new(pages),
        }
    }

    pub fn open_at_start(mut self, open: bool) -> Self {
        self.tutorial.open_at_start = open;
        self
    }
}

impl Plugin for FestivalUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.tutorial.clone())
            .add_systems(PreStartup, pick_ui_font)
            .add_systems(Startup, (spawn_tutorial_button, open_at_start))
            .add_systems(
                Update,
                (button_colors, tutorial_keys, sync_tutorial_overlay).chain(),
            );
    }
}
