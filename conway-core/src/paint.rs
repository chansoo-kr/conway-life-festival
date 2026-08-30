use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    camera::{MainCamera, cursor_cell},
    grid::{GridSize, PendingEdits},
    rle::Pattern,
    sim::GridView,
};

#[derive(Clone, Debug, Default)]
pub enum PaintMode {
    #[default]
    Pen,
    Stamp {
        name: String,
        pattern: Pattern,
    },
}

#[derive(Resource, Clone, Debug)]
pub struct PaintTool {
    pub enabled: bool,
    pub mode: PaintMode,
    pub player: u32,
}

impl Default for PaintTool {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: PaintMode::Pen,
            player: 0,
        }
    }
}

#[derive(Message, Clone, Debug)]
pub struct PaintRequest {
    pub cells: Vec<IVec2>,
    pub alive: bool,
    pub player: u32,
    pub stamp: bool,
}

#[derive(Resource, Default, Debug)]
pub struct PaintState {
    last: Option<IVec2>,
    pub hover: Option<IVec2>,
}

pub fn pointer_over_ui(nodes: &Query<&Interaction, With<Node>>) -> bool {
    nodes.iter().any(|i| *i != Interaction::None)
}

pub fn line_cells(from: IVec2, to: IVec2) -> Vec<IVec2> {
    let mut out = Vec::new();
    let d = (to - from).abs();
    let sx = if from.x < to.x { 1 } else { -1 };
    let sy = if from.y < to.y { 1 } else { -1 };
    let mut err = d.x - d.y;
    let mut p = from;
    loop {
        if p == to {
            break;
        }
        let e2 = 2 * err;
        if e2 > -d.y {
            err -= d.y;
            p.x += sx;
        }
        if e2 < d.x {
            err += d.x;
            p.y += sy;
        }
        out.push(p);
    }
    out
}

fn collect_paint(
    tool: Res<PaintTool>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<MainCamera>>,
    ui_nodes: Query<&Interaction, With<Node>>,
    mut state: ResMut<PaintState>,
    mut writer: MessageWriter<PaintRequest>,
) {
    let (cam_tf, projection) = camera.into_inner();
    state.hover = cursor_cell(&window, cam_tf, projection, &grid, &view);

    let left = buttons.pressed(MouseButton::Left);
    let right = buttons.pressed(MouseButton::Right);

    if !tool.enabled || (!left && !right) || pointer_over_ui(&ui_nodes) {
        state.last = None;
        return;
    }
    let Some(cell) = state.hover else {
        state.last = None;
        return;
    };

    let erase = right && !left;
    match (&tool.mode, erase) {
        (PaintMode::Stamp { pattern, .. }, false) => {
            if buttons.just_pressed(MouseButton::Left) {
                let origin = cell - (pattern.size() / 2).as_ivec2();
                let cells: Vec<IVec2> = pattern
                    .alive_cells()
                    .map(|c| origin + c.as_ivec2())
                    .filter(|c| grid.contains(*c))
                    .collect();
                writer.write(PaintRequest {
                    cells,
                    alive: true,
                    player: tool.player,
                    stamp: true,
                });
            }
            state.last = None;
        }
        _ => {
            let mut cells = match state.last {
                Some(last) if last != cell => line_cells(last, cell),
                Some(_) => Vec::new(),
                None => vec![cell],
            };
            cells.retain(|c| grid.contains(*c));
            state.last = Some(cell);
            if !cells.is_empty() {
                writer.write(PaintRequest {
                    cells,
                    alive: !erase,
                    player: tool.player,
                    stamp: false,
                });
            }
        }
    }
}

pub fn apply_paint_requests(
    grid: Res<GridSize>,
    mut reader: MessageReader<PaintRequest>,
    mut edits: ResMut<PendingEdits>,
) {
    for req in reader.read() {
        let owner = req.alive.then_some(req.player);
        for c in &req.cells {
            edits.set_cell(&grid, *c, owner);
        }
    }
}

pub struct PaintPlugin {
    pub auto_apply: bool,
}

impl Default for PaintPlugin {
    fn default() -> Self {
        Self { auto_apply: true }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaintSet;

impl Plugin for PaintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaintTool>()
            .init_resource::<PaintState>()
            .add_message::<PaintRequest>()
            .add_systems(Update, collect_paint.in_set(PaintSet));
        if self.auto_apply {
            app.add_systems(Update, apply_paint_requests.after(PaintSet));
        }
    }
}
