use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{PrimaryWindow, WindowResized},
};

use crate::{grid::GridSize, sim::GridView};

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CameraActivity {
    pub zoomed: bool,
}

#[derive(Resource, Clone, Debug)]
pub struct CameraControl {
    pub zoom: bool,
    pub pan: bool,
    pub zoom_speed: f32,
    pub min_scale: f32,
    pub max_scale: f32,
    pub pan_speed: f32,
}

impl Default for CameraControl {
    fn default() -> Self {
        Self {
            zoom: true,
            pan: true,
            zoom_speed: 0.15,
            min_scale: 0.02,
            max_scale: 100.0,
            pan_speed: 500.0,
        }
    }
}

pub fn fit_scale(grid: &GridSize, display_factor: f32, viewport: Vec2, margin: f32) -> f32 {
    let world = grid.size.as_vec2() * display_factor;
    (world.x / viewport.x.max(1.0)).max(world.y / viewport.y.max(1.0)) * margin
}

pub fn screen_to_world(screen: Vec2, viewport: Vec2, cam_pos: Vec2, scale: f32) -> Vec2 {
    let rel = Vec2::new(screen.x - viewport.x * 0.5, viewport.y * 0.5 - screen.y);
    cam_pos + rel * scale
}

pub fn world_to_cell(world: Vec2, grid: &GridSize, display_factor: f32) -> IVec2 {
    let half = grid.size.as_vec2() * 0.5;
    let cell = Vec2::new(
        world.x / display_factor + half.x,
        half.y - world.y / display_factor,
    );
    cell.floor().as_ivec2()
}

fn ortho_scale(projection: &Projection) -> f32 {
    match projection {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    }
}

pub fn cursor_cell(
    window: &Window,
    cam_transform: &Transform,
    projection: &Projection,
    grid: &GridSize,
    view: &GridView,
) -> Option<IVec2> {
    let cursor = window.cursor_position()?;
    let viewport = Vec2::new(window.width(), window.height());
    let world = screen_to_world(
        cursor,
        viewport,
        cam_transform.translation.truncate(),
        ortho_scale(projection),
    );
    Some(world_to_cell(world, grid, view.display_factor))
}

pub fn fit_camera(
    cam_transform: &mut Transform,
    projection: &mut Projection,
    grid: &GridSize,
    view: &GridView,
    window: &Window,
    left_margin_px: f32,
    top_margin_px: f32,
) {
    let viewport = Vec2::new(
        (window.width() - left_margin_px).max(1.0),
        (window.height() - top_margin_px).max(1.0),
    );
    let scale = fit_scale(grid, view.display_factor, viewport, 1.06);
    if let Projection::Orthographic(o) = projection {
        o.scale = scale;
    }
    cam_transform.translation.x = -left_margin_px * 0.5 * scale;
    cam_transform.translation.y = top_margin_px * 0.5 * scale;
}

pub fn zoom_at_cursor(
    control: Res<CameraControl>,
    scroll: Res<AccumulatedMouseScroll>,
    nodes: Query<&Interaction, With<Node>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
    mut activity: ResMut<CameraActivity>,
) {
    if !control.zoom || scroll.delta.y == 0.0 || crate::paint::pointer_over_ui(&nodes) {
        return;
    }
    activity.zoomed = true;
    let (mut transform, mut projection) = camera.into_inner();
    let Projection::Orthographic(o) = &mut *projection else {
        return;
    };
    let old = o.scale;
    let new = (old * (1.0 - scroll.delta.y * control.zoom_speed))
        .clamp(control.min_scale, control.max_scale);
    if (new - old).abs() < f32::EPSILON {
        return;
    }
    o.scale = new;

    if let Some(cursor) = window.cursor_position() {
        let viewport = Vec2::new(window.width(), window.height());
        let rel = Vec2::new(cursor.x - viewport.x * 0.5, viewport.y * 0.5 - cursor.y);
        let delta = rel * (old - new);
        transform.translation += delta.extend(0.0);
    }
}

pub fn pan_camera(
    control: Res<CameraControl>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    camera: Single<(&mut Transform, &Projection), With<MainCamera>>,
) {
    if !control.pan {
        return;
    }
    let (mut transform, projection) = camera.into_inner();
    let scale = ortho_scale(projection);

    let mut delta = Vec2::ZERO;

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }
    if dir != Vec2::ZERO {
        delta += dir.normalize() * control.pan_speed * scale * time.delta_secs();
    }

    if buttons.pressed(MouseButton::Middle) && motion.delta != Vec2::ZERO {
        delta += Vec2::new(-motion.delta.x, motion.delta.y) * scale;
    }

    if delta != Vec2::ZERO {
        transform.translation += delta.extend(0.0);
    }
}

pub struct CameraControlPlugin;

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraControl>()
            .init_resource::<CameraActivity>()
            .add_systems(Update, (zoom_at_cursor, pan_camera));
    }
}

#[derive(Resource, Clone, Copy)]
pub struct FitMargins {
    pub left: f32,
    pub top: f32,
}

fn fit_to_window(
    margins: Res<FitMargins>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let (mut tf, mut proj) = camera.into_inner();
    fit_camera(
        &mut tf,
        &mut proj,
        &grid,
        &view,
        &window,
        margins.left,
        margins.top,
    );
}

fn window_resized(mut resized: MessageReader<WindowResized>) -> bool {
    resized.read().last().is_some()
}

pub struct FitCameraPlugin {
    pub left_margin: f32,
    pub top_margin: f32,
}

impl Plugin for FitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FitMargins {
            left: self.left_margin,
            top: self.top_margin,
        })
        .add_systems(PostStartup, fit_to_window)
        .add_systems(Update, fit_to_window.run_if(window_resized));
    }
}
