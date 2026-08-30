use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::schedule::camera_driver,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{storage_buffer_read_only_sized, storage_buffer_sized},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderGraph},
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
    shader::{ShaderCacheError, ShaderDefVal, ShaderRef},
    sprite_render::{Material2d, Material2dPlugin},
    window::PrimaryWindow,
    winit::{UpdateMode, WinitSettings},
};

use crate::{
    camera::{MainCamera, fit_scale},
    grid::{GridSize, PendingEdits, alive_cells_in, bytes_to_words, count_plane, words_to_bytes},
};

const COMPUTE_SHADER_PATH: &str = "shaders/conway_compute.wgsl";
const BATTLE_COMPUTE_SHADER_PATH: &str = "shaders/conway_battle_compute.wgsl";
const EDIT_SHADER_PATH: &str = "shaders/conway_edit.wgsl";
const GRID_SHADER_PATH: &str = "shaders/conway_grid.wgsl";

const WORKGROUP_SIZE: u32 = 8;
const EDIT_WORKGROUP_SIZE: u32 = 64;

#[derive(Clone, Debug)]
pub struct GridColors {
    pub line: LinearRgba,
    pub dead: LinearRgba,
    pub p1: LinearRgba,
    pub p2: LinearRgba,
}

impl Default for GridColors {
    fn default() -> Self {
        Self {
            line: LinearRgba::new(0.15, 0.16, 0.19, 1.0),
            dead: LinearRgba::new(0.08, 0.09, 0.11, 1.0),
            p1: LinearRgba::rgb(0.35, 0.85, 0.55),
            p2: LinearRgba::rgb(0.95, 0.45, 0.35),
        }
    }
}

#[derive(Clone, Debug)]
pub enum InitialView {
    FitGrid { left_margin: f32, top_margin: f32 },
    CellsWide(f32),
}

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub size: UVec2,
    pub players: u32,
    pub display_factor: f32,
    pub initial_rate: f32,
    pub start_paused: bool,
    pub max_steps_per_frame: u32,
    pub readback: bool,
    pub colors: GridColors,
    pub initial_view: InitialView,
    pub target_fps: f64,
    pub initial_words: Option<Vec<u32>>,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            size: UVec2::new(320, 180),
            players: 1,
            display_factor: 3.0,
            initial_rate: 10.0,
            start_paused: false,
            max_steps_per_frame: 2048,
            readback: false,
            colors: GridColors::default(),
            initial_view: InitialView::FitGrid {
                left_margin: 0.0,
                top_margin: 0.0,
            },
            target_fps: 120.0,
            initial_words: None,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct GridView {
    pub display_factor: f32,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SimSpeed {
    pub rate: f32,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SimControl {
    pub paused: bool,
    pub step_once: bool,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Generation(pub u64);

#[derive(Message, Clone, Debug)]
pub struct ResetGrid(pub Vec<u32>);

impl ResetGrid {
    pub fn empty(grid: &GridSize) -> Self {
        Self(vec![0; grid.total_words()])
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct GridSnapshot {
    pub words: Vec<u32>,
    pub received: u64,
}

impl GridSnapshot {
    pub fn is_ready(&self) -> bool {
        self.received > 0
    }

    pub fn count(&self, grid: &GridSize, plane: u32) -> u32 {
        count_plane(&self.words, grid, plane)
    }

    pub fn alive_cells(&self, grid: &GridSize, plane: u32) -> Vec<IVec2> {
        alive_cells_in(&self.words, grid, plane)
    }

    pub fn get(&self, grid: &GridSize, plane: u32, cell: IVec2) -> bool {
        if !grid.contains(cell) {
            return false;
        }
        let idx = grid.word_index(plane, cell.x as u32, cell.y as u32) as usize;
        self.words
            .get(idx)
            .is_some_and(|w| (w >> (cell.x as u32 % 32)) & 1 == 1)
    }
}

#[derive(Resource, Clone)]
pub struct SimReady(Arc<AtomicBool>);

impl SimReady {
    pub fn get(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Resource, Clone, Default)]
pub struct SimStats {
    dispatched: Arc<std::sync::atomic::AtomicU64>,
    render_frames: Arc<std::sync::atomic::AtomicU64>,
}

impl SimStats {
    pub fn dispatched(&self) -> u64 {
        self.dispatched.load(Ordering::Relaxed)
    }
    pub fn render_frames(&self) -> u64 {
        self.render_frames.load(Ordering::Relaxed)
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimSet;

#[derive(Component)]
pub struct GridDisplay;

#[derive(Resource, Clone)]
struct SimConfigRes(SimConfig);

#[derive(Resource)]
struct SimTimer(Timer);

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
struct StepsThisFrame(u32);

#[derive(Resource, Clone, ExtractResource)]
pub struct ConwayBuffers {
    pub buffer_a: Handle<ShaderBuffer>,
    pub buffer_b: Handle<ShaderBuffer>,
}

#[derive(Resource)]
pub struct ConwayMaterials {
    pub a: Handle<ConwayGridMaterial>,
    pub b: Handle<ConwayGridMaterial>,
}

#[derive(Component)]
struct ReadbackTarget;

#[derive(ShaderType, Debug, Clone)]
pub struct GridShaderParams {
    pub grid_size: Vec2,
    pub line_width: f32,
    pub players: u32,
    pub line_color: LinearRgba,
    pub dead_color: LinearRgba,
    pub p1_color: LinearRgba,
    pub p2_color: LinearRgba,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ConwayGridMaterial {
    #[uniform(0)]
    pub params: GridShaderParams,
    #[storage(1, read_only)]
    pub cells: Handle<ShaderBuffer>,
}

impl Material2d for ConwayGridMaterial {
    fn fragment_shader() -> ShaderRef {
        GRID_SHADER_PATH.into()
    }
}

pub struct ConwaySimPlugin(pub SimConfig);

impl Plugin for ConwaySimPlugin {
    fn build(&self, app: &mut App) {
        let cfg = &self.0;
        let grid = GridSize::new(cfg.size, cfg.players);
        let ready = SimReady(Arc::new(AtomicBool::new(false)));
        let stats = SimStats::default();

        app.insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.08)))
            .insert_resource(WinitSettings {
                focused_mode: UpdateMode::reactive(Duration::from_secs_f64(1.0 / cfg.target_fps)),
                unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs_f64(1.0 / 5.0)),
            })
            .insert_resource(grid)
            .insert_resource(GridView {
                display_factor: cfg.display_factor,
            })
            .insert_resource(SimSpeed {
                rate: cfg.initial_rate,
            })
            .insert_resource(SimControl {
                paused: cfg.start_paused,
                step_once: false,
            })
            .insert_resource(SimTimer(Timer::from_seconds(
                1.0 / cfg.initial_rate.max(0.001),
                TimerMode::Repeating,
            )))
            .insert_resource(SimConfigRes(cfg.clone()))
            .insert_resource(ready.clone())
            .insert_resource(stats.clone())
            .init_resource::<Generation>()
            .init_resource::<PendingEdits>()
            .init_resource::<GridSnapshot>()
            .init_resource::<StepsThisFrame>()
            .add_message::<ResetGrid>()
            .add_plugins((
                Material2dPlugin::<ConwayGridMaterial>::default(),
                ExtractResourcePlugin::<ConwayBuffers>::default(),
                ExtractResourcePlugin::<StepsThisFrame>::default(),
                ExtractResourcePlugin::<PendingEdits>::default(),
            ))
            .add_systems(Startup, setup)
            .add_systems(First, clear_pending_edits)
            .add_systems(
                Update,
                (
                    apply_reset_grid,
                    sync_speed_timer,
                    tick_simulation,
                    switch_display_buffer,
                    update_readback_target,
                    update_grid_lines,
                )
                    .chain()
                    .in_set(SimSet),
            );

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .insert_resource(grid)
            .insert_resource(ready)
            .insert_resource(stats)
            .init_resource::<ConwayState>()
            .add_systems(RenderStartup, init_conway_pipeline)
            .add_systems(
                Render,
                (prepare_bind_group, prepare_edit_bind_groups)
                    .chain()
                    .in_set(RenderSystems::PrepareBindGroups),
            )
            .add_systems(Render, update_state.in_set(RenderSystems::Prepare))
            .add_systems(RenderGraph, run_conway_compute.before(camera_driver));
    }
}

fn setup(
    mut commands: Commands,
    cfg: Res<SimConfigRes>,
    grid: Res<GridSize>,
    view: Res<GridView>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ConwayGridMaterial>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let cfg = &cfg.0;
    let words = cfg
        .initial_words
        .clone()
        .unwrap_or_else(|| vec![0; grid.total_words()]);
    assert_eq!(
        words.len(),
        grid.total_words(),
        "initial_words length does not match the grid"
    );
    let data = words_to_bytes(&words);

    let buffer_a = buffers.add(ShaderBuffer::new(&data, RenderAssetUsages::default()));
    let buffer_b = buffers.add(ShaderBuffer::new(&data, RenderAssetUsages::default()));

    let params = GridShaderParams {
        grid_size: grid.size.as_vec2(),
        line_width: 0.05,
        players: grid.planes,
        line_color: cfg.colors.line,
        dead_color: cfg.colors.dead,
        p1_color: cfg.colors.p1,
        p2_color: cfg.colors.p2,
    };
    let material_a = materials.add(ConwayGridMaterial {
        params: params.clone(),
        cells: buffer_a.clone(),
    });
    let material_b = materials.add(ConwayGridMaterial {
        params,
        cells: buffer_b.clone(),
    });

    commands.spawn((
        GridDisplay,
        Mesh2d(meshes.add(Rectangle::new(grid.size.x as f32, grid.size.y as f32))),
        MeshMaterial2d(material_a.clone()),
        Transform::from_scale(Vec3::splat(view.display_factor)),
    ));

    let viewport = Vec2::new(window.width(), window.height());
    let (scale, translation) = match cfg.initial_view {
        InitialView::FitGrid {
            left_margin,
            top_margin,
        } => {
            let vp = Vec2::new(
                (viewport.x - left_margin).max(1.0),
                (viewport.y - top_margin).max(1.0),
            );
            let s = fit_scale(&grid, view.display_factor, vp, 1.06);
            (
                s,
                Vec3::new(-left_margin * 0.5 * s, top_margin * 0.5 * s, 0.0),
            )
        }
        InitialView::CellsWide(cells) => (
            cells * view.display_factor / viewport.x.max(1.0),
            Vec3::ZERO,
        ),
    };
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scale,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_translation(translation),
    ));

    if cfg.readback {
        commands
            .spawn((ReadbackTarget, Readback::buffer(buffer_a.clone())))
            .observe(on_readback);
    }

    commands.insert_resource(ConwayBuffers { buffer_a, buffer_b });
    commands.insert_resource(ConwayMaterials {
        a: material_a,
        b: material_b,
    });
}

fn on_readback(event: On<ReadbackComplete>, mut snapshot: ResMut<GridSnapshot>) {
    snapshot.words = bytes_to_words(&event.data);
    snapshot.received += 1;
}

fn clear_pending_edits(ready: Res<SimReady>, mut edits: ResMut<PendingEdits>) {
    if ready.get() && !edits.is_empty() {
        edits.clear();
    }
}

fn apply_reset_grid(
    mut reader: MessageReader<ResetGrid>,
    grid: Res<GridSize>,
    conway_buffers: Res<ConwayBuffers>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut generation: ResMut<Generation>,
) {
    let Some(reset) = reader.read().last() else {
        return;
    };
    if reset.0.len() != grid.total_words() {
        error!(
            "ResetGrid length mismatch: {} != {}",
            reset.0.len(),
            grid.total_words()
        );
        return;
    }
    let data = words_to_bytes(&reset.0);
    for handle in [&conway_buffers.buffer_a, &conway_buffers.buffer_b] {
        if let Some(mut buffer) = buffers.get_mut(handle) {
            *buffer = ShaderBuffer::new(&data, RenderAssetUsages::default());
        }
    }
    generation.0 = 0;
}

fn sync_speed_timer(speed: Res<SimSpeed>, mut timer: ResMut<SimTimer>) {
    if speed.is_changed() {
        timer.0 = Timer::from_seconds(1.0 / speed.rate.max(0.001), TimerMode::Repeating);
    }
}

fn tick_simulation(
    time: Res<Time>,
    ready: Res<SimReady>,
    cfg: Res<SimConfigRes>,
    mut control: ResMut<SimControl>,
    mut timer: ResMut<SimTimer>,
    mut steps: ResMut<StepsThisFrame>,
    mut generation: ResMut<Generation>,
) {
    let mut n = 0u32;
    if ready.get() {
        if control.step_once {
            n = 1;
        } else if !control.paused {
            timer.0.tick(time.delta());
            n = timer
                .0
                .times_finished_this_tick()
                .min(cfg.0.max_steps_per_frame);
        }
    }
    if control.step_once {
        control.step_once = false;
    }
    if steps.0 != n {
        steps.0 = n;
    }
    if n > 0 {
        generation.0 += n as u64;
    }
}

fn switch_display_buffer(
    steps: Res<StepsThisFrame>,
    conway_materials: Res<ConwayMaterials>,
    display: Single<&mut MeshMaterial2d<ConwayGridMaterial>, With<GridDisplay>>,
) {
    if steps.0.is_multiple_of(2) {
        return;
    }
    let mut display = display.into_inner();
    display.0 = if display.0 == conway_materials.a {
        conway_materials.b.clone()
    } else {
        conway_materials.a.clone()
    };
}

fn update_readback_target(
    display: Single<&MeshMaterial2d<ConwayGridMaterial>, With<GridDisplay>>,
    conway_materials: Res<ConwayMaterials>,
    conway_buffers: Res<ConwayBuffers>,
    mut targets: Query<&mut Readback, With<ReadbackTarget>>,
) {
    let current = if display.0 == conway_materials.a {
        &conway_buffers.buffer_a
    } else {
        &conway_buffers.buffer_b
    };
    for mut readback in &mut targets {
        let same = matches!(&*readback, Readback::Buffer { buffer, .. } if buffer == current);
        if !same {
            *readback = Readback::buffer(current.clone());
        }
    }
}

fn update_grid_lines(
    camera: Single<&Projection, With<MainCamera>>,
    view: Res<GridView>,
    conway_materials: Res<ConwayMaterials>,
    mut materials: ResMut<Assets<ConwayGridMaterial>>,
) {
    let Projection::Orthographic(orthographic) = &**camera else {
        return;
    };
    let new_width = (1.5 * orthographic.scale / view.display_factor).min(1.0);
    let Some(current) = materials.get(&conway_materials.a) else {
        return;
    };
    if (current.params.line_width - new_width).abs() < 1e-5 {
        return;
    }
    for handle in [&conway_materials.a, &conway_materials.b] {
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.line_width = new_width;
        }
    }
}

#[derive(Resource)]
struct ConwayPipeline {
    layout: BindGroupLayoutDescriptor,
    update_pipeline: CachedComputePipelineId,
    edit_layout: BindGroupLayoutDescriptor,
    edit_pipeline: CachedComputePipelineId,
}

#[derive(Resource)]
struct ConwayBindGroups {
    groups: [BindGroup; 2],
    buffer_ids: (BufferId, BufferId),
}

#[derive(Resource)]
struct EditBindGroups {
    groups: [BindGroup; 2],
    count: u32,
}

#[derive(Resource, Default)]
enum ConwayState {
    #[default]
    Loading,
    Update(usize),
}

fn init_conway_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
    grid: Res<GridSize>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "ConwayBuffers",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
            ),
        ),
    );
    let shader_defs = vec![
        ShaderDefVal::UInt("WORDS_X".into(), grid.words_x()),
        ShaderDefVal::UInt("GRID_H".into(), grid.size.y),
    ];
    let shader_path = if grid.planes == 2 {
        BATTLE_COMPUTE_SHADER_PATH
    } else {
        COMPUTE_SHADER_PATH
    };
    let update_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("conway_update".into()),
        layout: vec![layout.clone()],
        shader: asset_server.load(shader_path),
        shader_defs,
        entry_point: Some(Cow::from("update")),
        ..default()
    });

    let edit_layout = BindGroupLayoutDescriptor::new(
        "ConwayEdits",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_sized(false, None),
                storage_buffer_read_only_sized(false, None),
            ),
        ),
    );
    let edit_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("conway_apply_edits".into()),
        layout: vec![edit_layout.clone()],
        shader: asset_server.load(EDIT_SHADER_PATH),
        entry_point: Some(Cow::from("apply_edits")),
        ..default()
    });

    commands.insert_resource(ConwayPipeline {
        layout,
        update_pipeline,
        edit_layout,
        edit_pipeline,
    });
}

fn prepare_bind_group(
    mut commands: Commands,
    pipeline: Res<ConwayPipeline>,
    gpu_buffers: Res<RenderAssets<GpuShaderBuffer>>,
    conway_buffers: Res<ConwayBuffers>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    existing: Option<Res<ConwayBindGroups>>,
) {
    let (Some(buf_a), Some(buf_b)) = (
        gpu_buffers.get(&conway_buffers.buffer_a),
        gpu_buffers.get(&conway_buffers.buffer_b),
    ) else {
        return;
    };
    let buffer_ids = (buf_a.buffer.id(), buf_b.buffer.id());
    if let Some(existing) = &existing
        && existing.buffer_ids == buffer_ids
    {
        return;
    }
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
    let bind_group_0 = render_device.create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((
            buf_a.buffer.as_entire_binding(),
            buf_b.buffer.as_entire_binding(),
        )),
    );
    let bind_group_1 = render_device.create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((
            buf_b.buffer.as_entire_binding(),
            buf_a.buffer.as_entire_binding(),
        )),
    );
    commands.insert_resource(ConwayBindGroups {
        groups: [bind_group_0, bind_group_1],
        buffer_ids,
    });
}

fn prepare_edit_bind_groups(
    mut commands: Commands,
    pipeline: Res<ConwayPipeline>,
    edits: Option<Res<PendingEdits>>,
    gpu_buffers: Res<RenderAssets<GpuShaderBuffer>>,
    conway_buffers: Res<ConwayBuffers>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    existing: Option<Res<EditBindGroups>>,
) {
    let Some(edits) = edits.filter(|e| !e.is_empty()) else {
        if existing.is_some() {
            commands.remove_resource::<EditBindGroups>();
        }
        return;
    };
    let (Some(buf_a), Some(buf_b)) = (
        gpu_buffers.get(&conway_buffers.buffer_a),
        gpu_buffers.get(&conway_buffers.buffer_b),
    ) else {
        return;
    };
    let bytes = edits.to_bytes();
    let edit_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("conway_edits"),
        contents: &bytes,
        usage: BufferUsages::STORAGE,
    });
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.edit_layout);
    let for_a = render_device.create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((
            buf_a.buffer.as_entire_binding(),
            edit_buffer.as_entire_binding(),
        )),
    );
    let for_b = render_device.create_bind_group(
        None,
        &layout,
        &BindGroupEntries::sequential((
            buf_b.buffer.as_entire_binding(),
            edit_buffer.as_entire_binding(),
        )),
    );
    commands.insert_resource(EditBindGroups {
        groups: [for_a, for_b],
        count: edits.len() as u32,
    });
}

fn update_state(
    pipeline: Res<ConwayPipeline>,
    pipeline_cache: Res<PipelineCache>,
    ready: Res<SimReady>,
    mut state: ResMut<ConwayState>,
) {
    if !matches!(*state, ConwayState::Loading) {
        return;
    }
    let mut all_ok = true;
    for (id, name) in [
        (pipeline.update_pipeline, "update"),
        (pipeline.edit_pipeline, "apply_edits"),
    ] {
        match pipeline_cache.get_compute_pipeline_state(id) {
            CachedPipelineState::Ok(_) => {}
            CachedPipelineState::Err(ShaderCacheError::ShaderNotLoaded(_)) => all_ok = false,
            CachedPipelineState::Err(err) => {
                panic!("failed to create compute pipeline '{name}':\n{err}")
            }
            _ => all_ok = false,
        }
    }
    if all_ok {
        *state = ConwayState::Update(0);
        ready.0.store(true, Ordering::Release);
    }
}

fn run_conway_compute(
    mut render_context: RenderContext,
    bind_groups: Option<Res<ConwayBindGroups>>,
    edit_groups: Option<Res<EditBindGroups>>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<ConwayPipeline>,
    grid: Res<GridSize>,
    stats: Res<SimStats>,
    mut state: ResMut<ConwayState>,
    steps: Res<StepsThisFrame>,
) {
    stats.render_frames.fetch_add(1, Ordering::Relaxed);
    let Some(bind_groups) = bind_groups else {
        return;
    };
    let ConwayState::Update(start_index) = *state else {
        return;
    };
    if edit_groups.is_none() && steps.0 == 0 {
        return;
    }
    stats
        .dispatched
        .fetch_add(steps.0 as u64, Ordering::Relaxed);

    let wg_x = grid.words_x().div_ceil(WORKGROUP_SIZE);
    let wg_y = grid.size.y.div_ceil(WORKGROUP_SIZE);

    let mut pass = render_context
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("conway"),
            ..default()
        });

    let mut index = start_index;

    if let Some(edits) = edit_groups
        && let Some(edit_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.edit_pipeline)
    {
        pass.set_pipeline(edit_pipeline);
        pass.set_bind_group(0, &edits.groups[index], &[]);
        pass.dispatch_workgroups(edits.count.div_ceil(EDIT_WORKGROUP_SIZE), 1, 1);
    }

    if steps.0 > 0
        && let Some(update_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.update_pipeline)
    {
        pass.set_pipeline(update_pipeline);
        for _ in 0..steps.0 {
            pass.set_bind_group(0, &bind_groups.groups[index], &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
            index = 1 - index;
        }
    }
    drop(pass);

    if index != start_index {
        *state = ConwayState::Update(index);
    }
}
