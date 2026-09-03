use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    text::{FontCx, FontWeight},
};

use crate::camera::CameraActivity;

#[derive(Resource, Clone)]
pub struct UiFont {
    pub regular: FontSource,
    pub bold: FontSource,
    pub bold_weight: FontWeight,
}

const BUNDLED_FONTS: &[(&str, &str)] = &[
    ("fonts/ui.ttf", "fonts/ui-bold.ttf"),
    (
        "fonts/IBMPlexSansKR-Regular.ttf",
        "fonts/IBMPlexSansKR-SemiBold.ttf",
    ),
];

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
    let asset_root = std::path::PathBuf::from(crate::asset_root());
    for (regular, bold) in BUNDLED_FONTS {
        if !asset_root.join(regular).is_file() {
            continue;
        }
        let bold = if asset_root.join(bold).is_file() {
            bold
        } else {
            regular
        };
        info!("ui font: assets/{regular} (bold: {bold})");
        commands.insert_resource(UiFont {
            regular: FontSource::Handle(asset_server.load(*regular)),
            bold: FontSource::Handle(asset_server.load(*bold)),
            bold_weight: FontWeight::NORMAL,
        });
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
            let family = FontSource::Family(found.clone().into());
            commands.insert_resource(UiFont {
                regular: family.clone(),
                bold: family,
                bold_weight: FontWeight::SEMIBOLD,
            });
            return;
        }
    }
    warn!("no Korean system font found; add assets/fonts/ui.ttf (falling back to sans-serif)");
    commands.insert_resource(UiFont {
        regular: FontSource::SansSerif,
        bold: FontSource::SansSerif,
        bold_weight: FontWeight::SEMIBOLD,
    });
}

pub const PANEL_BG: Color = Color::srgba(0.075, 0.075, 0.08, 0.94);
pub const CARD_BG: Color = Color::srgb(0.10, 0.10, 0.11);
pub const INSET_BG: Color = Color::srgb(0.05, 0.05, 0.055);
pub const LINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);
pub const TEXT_COLOR: Color = Color::srgb(0.93, 0.93, 0.92);
pub const MUTED_COLOR: Color = Color::srgb(0.62, 0.62, 0.60);
pub const ACCENT: Color = Color::srgb(0.35, 0.85, 0.55);
pub const RADIUS_BUTTON: f32 = 2.0;
pub const RADIUS_CARD: f32 = 3.0;
pub const TOP_RIGHT_RESERVE: f32 = 150.0;

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
            normal: Color::srgb(0.155, 0.155, 0.17),
            hovered: Color::srgb(0.22, 0.22, 0.24),
            pressed: Color::srgb(0.30, 0.52, 0.40),
            selected: Color::srgb(0.20, 0.44, 0.33),
        }
    }
}

impl ButtonColors {
    pub fn accent() -> Self {
        Self {
            normal: Color::srgb(0.17, 0.44, 0.31),
            hovered: Color::srgb(0.23, 0.56, 0.39),
            pressed: Color::srgb(0.33, 0.73, 0.49),
            selected: Color::srgb(0.28, 0.63, 0.44),
        }
    }
    pub fn danger() -> Self {
        Self {
            normal: Color::srgb(0.42, 0.18, 0.19),
            hovered: Color::srgb(0.55, 0.24, 0.25),
            pressed: Color::srgb(0.73, 0.33, 0.33),
            selected: Color::srgb(0.63, 0.28, 0.29),
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
            font: font.regular.clone(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

pub fn title_text(font: &UiFont, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.bold.clone(),
            font_size: FontSize::Px(size),
            weight: font.bold_weight,
            ..default()
        },
        TextColor(color),
    )
}

pub fn body_text(font: &UiFont, s: impl Into<String>, size: f32) -> impl Bundle {
    (
        hud_text(font, s, size, TEXT_COLOR),
        TextLayout::linebreak(LineBreak::WordOrCharacter),
        Node {
            max_width: percent(100),
            ..default()
        },
    )
}

pub fn divider() -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: px(1),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(LINE),
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
        node.padding = UiRect::axes(px(12), px(7));
    }
    if node.border == UiRect::default() {
        node.border = UiRect::all(px(1));
    }
    if node.border_radius == BorderRadius::default() {
        node.border_radius = BorderRadius::all(px(RADIUS_BUTTON));
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
        BorderColor::all(LINE),
        colors,
        children![hud_text(font, label, size, TEXT_COLOR)],
    )
}

pub fn panel(mut node: Node) -> impl Bundle {
    if node.border == UiRect::default() {
        node.border = UiRect::all(px(1));
    }
    (
        node,
        BackgroundColor(PANEL_BG),
        BorderColor::all(LINE),
        Interaction::None,
    )
}

pub fn card_frame(mut node: Node) -> impl Bundle {
    if node.border == UiRect::default() {
        node.border = UiRect::new(px(1), px(1), px(3), px(1));
    }
    if node.border_radius == BorderRadius::default() {
        node.border_radius = BorderRadius::all(px(RADIUS_CARD));
    }
    (
        node,
        BackgroundColor(CARD_BG),
        BorderColor {
            top: ACCENT,
            right: LINE,
            bottom: LINE,
            left: LINE,
        },
        Interaction::None,
    )
}

pub fn card(width: f32) -> impl Bundle {
    card_frame(Node {
        width: px(width),
        max_width: percent(92),
        flex_direction: FlexDirection::Column,
        padding: UiRect::axes(px(28), px(24)),
        row_gap: px(12),
        ..default()
    })
}

pub fn overlay_root(modal: bool) -> impl Bundle {
    (
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

#[derive(Resource, Clone, Debug)]
pub struct Intro {
    pub mode_name: String,
    pub mode_description: String,
    pub hint: String,
    pub open: bool,
    pub just_closed: bool,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct IntroDismissed;

#[derive(Message, Clone, Copy, Debug)]
pub struct SessionReset;

pub fn intro_closed(intro: Res<Intro>) -> bool {
    !intro.open && !intro.just_closed
}

pub fn intro_open(intro: Res<Intro>) -> bool {
    intro.open
}

#[derive(Component)]
struct IntroOverlay;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiSet;

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
    intro: Res<Intro>,
    mut clock: ResMut<IdleClock>,
    mut reset: MessageWriter<SessionReset>,
) {
    let now = time.elapsed_secs_f64();
    let input = keys.get_just_pressed().next().is_some()
        || buttons.get_just_pressed().next().is_some()
        || motion.delta != Vec2::ZERO
        || scroll.delta != Vec2::ZERO;
    if input || intro.open {
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
    mut intro: ResMut<Intro>,
    mut activity: ResMut<CameraActivity>,
) {
    if resets.read().last().is_none() {
        return;
    }
    intro.open = true;
    *activity = CameraActivity::default();
}

fn spawn_home_button(mut commands: Commands, font: Res<UiFont>) {
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
            bar.spawn(button(&font, "처음으로", 18.0)).observe(
                |_: On<Pointer<Click>>, mut reset: MessageWriter<SessionReset>| {
                    reset.write(SessionReset);
                },
            );
        });
}

fn dismiss_intro(intro: &mut Intro, dismissed: &mut MessageWriter<IntroDismissed>) {
    if intro.open {
        intro.open = false;
        intro.just_closed = true;
        dismissed.write(IntroDismissed);
    }
}

fn clear_just_closed(mut intro: ResMut<Intro>) {
    if intro.just_closed {
        intro.just_closed = false;
    }
}

fn intro_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut intro: ResMut<Intro>,
    mut dismissed: MessageWriter<IntroDismissed>,
) {
    if intro.open && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space)) {
        dismiss_intro(&mut intro, &mut dismissed);
    }
}

fn spawn_intro(commands: &mut Commands, intro: &Intro, font: &UiFont) {
    commands
        .spawn((overlay_root(true), IntroOverlay))
        .with_children(|root| {
            root.spawn(card(720.0)).with_children(|card| {
                card.spawn(hud_text(font, "생명 게임 축제", 14.0, MUTED_COLOR));
                card.spawn(title_text(font, intro.mode_name.clone(), 30.0, TEXT_COLOR));
                card.spawn(divider());
                card.spawn(body_text(font, intro.mode_description.clone(), 19.0));
                if !intro.hint.is_empty() {
                    card.spawn(hud_text(font, intro.hint.clone(), 16.0, MUTED_COLOR));
                }
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(10),
                    margin: UiRect::top(px(8)),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(button_with(
                        font,
                        "시작 (Enter)",
                        18.0,
                        ButtonColors::accent(),
                    ))
                    .observe(
                        |_: On<Pointer<Click>>,
                         mut intro: ResMut<Intro>,
                         mut dismissed: MessageWriter<IntroDismissed>| {
                            dismiss_intro(&mut intro, &mut dismissed);
                        },
                    );
                });
            });
        });
}

fn sync_intro_overlay(
    mut commands: Commands,
    intro: Res<Intro>,
    font: Res<UiFont>,
    existing: Query<Entity, With<IntroOverlay>>,
    mut last_open: Local<Option<bool>>,
) {
    if *last_open == Some(intro.open) {
        return;
    }
    *last_open = Some(intro.open);
    for e in &existing {
        commands.entity(e).despawn();
    }
    if intro.open {
        spawn_intro(&mut commands, &intro, &font);
    }
}

pub struct FestivalUiPlugin {
    pub intro: Intro,
    pub idle_reset_secs: f32,
}

impl FestivalUiPlugin {
    pub fn new(mode_name: impl Into<String>, mode_description: impl Into<String>) -> Self {
        Self {
            intro: Intro {
                mode_name: mode_name.into(),
                mode_description: mode_description.into(),
                hint: String::new(),
                open: true,
                just_closed: false,
            },
            idle_reset_secs: idle_reset_from_args(180.0),
        }
    }

    pub fn intro_hint(mut self, hint: impl Into<String>) -> Self {
        self.intro.hint = hint.into();
        self
    }

    pub fn idle_reset(mut self, secs: f32) -> Self {
        self.idle_reset_secs = secs;
        self
    }
}

impl Plugin for FestivalUiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.intro.clone())
            .insert_resource(SessionConfig {
                idle_reset_secs: self.idle_reset_secs,
            })
            .init_resource::<IdleClock>()
            .init_resource::<CameraActivity>()
            .add_message::<IntroDismissed>()
            .add_message::<SessionReset>()
            .add_systems(PreStartup, pick_ui_font)
            .add_systems(First, clear_just_closed)
            .add_systems(Startup, spawn_home_button)
            .add_systems(
                Update,
                (
                    button_colors,
                    idle_watch,
                    apply_session_reset,
                    intro_keys,
                    sync_intro_overlay,
                )
                    .chain()
                    .in_set(UiSet),
            );
    }
}
