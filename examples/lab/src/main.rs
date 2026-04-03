#[cfg(feature = "e2e")]
mod e2e;
#[cfg(feature = "e2e")]
mod scenarios;

use saddle_ai_fov_example_support as support;

use bevy::prelude::*;
#[cfg(feature = "dev")]
use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
#[cfg(feature = "dev")]
use bevy_brp_extras::BrpExtrasPlugin;
use saddle_ai_fov::{
    FovDebugSettings, FovDirty, FovOccluder, FovPerceptionModifiers, FovPlugin, FovTarget,
    GridFov, GridFovState, GridOpacityMap, OccluderShape, SpatialAwarenessConfig, SpatialFov,
    SpatialFovState,
};
use saddle_pane::prelude::*;
use support::{GridCellSprite, apply_grid_visibility_colors, spawn_grid_tiles};

pub const MEMORY_SAMPLE_CELL: IVec2 = IVec2::new(2, 8);
const GRID_PATH: &[IVec2] = &[
    IVec2::new(2, 8),
    IVec2::new(2, 2),
    IVec2::new(7, 2),
    IVec2::new(7, 6),
    IVec2::new(12, 6),
    IVec2::new(12, 2),
];
const GRID_SPEED: f32 = 0.36;
const FRONT_TARGET_NAME: &str = "Front Target";
const AWARENESS_TARGET_NAME: &str = "Awareness Target";
pub const HIDDEN_TARGET_NAME: &str = "Hidden Target";

#[derive(Component)]
struct LabGridViewer;

#[derive(Component)]
struct LabGuard;

#[derive(Component)]
struct LabOverlay;

#[derive(Component)]
struct TargetVisual;

#[derive(Resource, Clone, Copy)]
struct LabEntities {
    grid_viewer: Entity,
    guard: Entity,
    awareness_target: Entity,
}

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "FOV Lab Controls", position = "top-right")]
pub struct LabControl {
    #[pane]
    pub pause_motion: bool,
    #[pane(skip)]
    pub grid_progress: f32,
    #[pane(slider, min = -1.4, max = 1.4, step = 0.02)]
    pub guard_angle: f32,
    #[pane(slider, min = 2.0, max = 8.0, step = 1.0)]
    pub grid_radius: i32,
    #[pane(slider, min = 0.1, max = 1.2, step = 0.02)]
    pub grid_speed: f32,
    #[pane(slider, min = 180.0, max = 560.0, step = 5.0)]
    pub guard_range: f32,
    #[pane(slider, min = 0.2, max = 1.2, step = 0.02)]
    pub guard_half_angle: f32,
    #[pane(slider, min = 0.08, max = 0.8, step = 0.02)]
    pub focused_half_angle: f32,
    #[pane(slider, min = 0.0, max = 96.0, step = 2.0)]
    pub near_override: f32,
    #[pane(slider, min = 0.2, max = 0.95, step = 0.01)]
    pub alert_threshold: f32,
    #[pane(slider, min = 0.1, max = 1.2, step = 0.02)]
    pub guard_sweep_speed: f32,
}

impl Default for LabControl {
    fn default() -> Self {
        Self {
            pause_motion: false,
            grid_progress: 0.0,
            guard_angle: 0.0,
            grid_radius: 4,
            grid_speed: GRID_SPEED,
            guard_range: 420.0,
            guard_half_angle: 0.56,
            focused_half_angle: 0.24,
            near_override: 42.0,
            alert_threshold: 0.8,
            guard_sweep_speed: 0.6,
        }
    }
}

#[derive(Resource, Debug, Clone, Default, Pane)]
#[pane(title = "FOV Lab Stats", position = "bottom-right")]
pub struct LabDiagnostics {
    #[pane(monitor)]
    pub grid_visible_cells: usize,
    #[pane(monitor)]
    pub grid_explored_cells: usize,
    #[pane(monitor)]
    pub memory_sample_visible: bool,
    #[pane(monitor)]
    pub memory_sample_explored: bool,
    #[pane(monitor)]
    pub guard_visible_targets: usize,
    #[pane(monitor)]
    pub remembered_targets: usize,
    #[pane(monitor)]
    pub front_target_visible: bool,
    #[pane(monitor)]
    pub front_target_awareness: f32,
    #[pane(monitor)]
    pub awareness_target_visible: bool,
    #[pane(monitor)]
    pub awareness_target_awareness: f32,
    #[pane(monitor)]
    pub hidden_target_visible: bool,
}

fn main() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.025, 0.03, 0.035)));
    app.insert_resource(LabControl::default());
    app.insert_resource(LabDiagnostics::default());
    app.insert_resource(lab_grid_map());
    app.insert_resource(FovDebugSettings {
        enabled: true,
        draw_grid_cells: true,
        draw_view_shapes: true,
        draw_occlusion_rays: true,
        max_grid_cells_per_viewer: 120,
    });
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "fov crate-local lab".into(),
            resolution: (1520, 900).into(),
            ..default()
        }),
        ..default()
    }));
    app.init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>();
    #[cfg(feature = "dev")]
    app.add_plugins(RemotePlugin::default());
    #[cfg(feature = "dev")]
    app.add_plugins(BrpExtrasPlugin::with_http_plugin(
        RemoteHttpPlugin::default(),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::FovLabE2EPlugin);
    app.add_plugins(FovPlugin::default());
    app.add_plugins((
        bevy_flair::FlairPlugin,
        bevy_input_focus::InputDispatchPlugin,
        bevy_ui_widgets::UiWidgetsPlugins,
        bevy_input_focus::tab_navigation::TabNavigationPlugin,
        PanePlugin,
    ))
        .register_pane::<LabControl>()
        .register_pane::<LabDiagnostics>();
    app.add_systems(Startup, setup);
    app.add_systems(
        Update,
        sync_lab_settings.before(saddle_ai_fov::FovSystems::MarkDirty),
    );
    app.add_systems(
        Update,
        animate_grid_viewer.before(saddle_ai_fov::FovSystems::MarkDirty),
    );
    app.add_systems(
        Update,
        animate_guard.before(saddle_ai_fov::FovSystems::MarkDirty),
    );
    app.add_systems(
        Update,
        tint_grid.after(saddle_ai_fov::FovSystems::Recompute),
    );
    app.add_systems(
        Update,
        tint_targets.after(saddle_ai_fov::FovSystems::Recompute),
    );
    app.add_systems(
        Update,
        update_diagnostics.after(saddle_ai_fov::FovSystems::Recompute),
    );
    app.add_systems(
        Update,
        update_overlay.after(saddle_ai_fov::FovSystems::Recompute),
    );
    app.run();
}

fn lab_grid_map() -> GridOpacityMap {
    let cell_size = 30.0;
    let height = support::DEMO_GRID.len() as u32;
    let width = support::DEMO_GRID[0].len() as u32;
    let spec = saddle_ai_fov::GridMapSpec {
        origin: Vec2::new(-680.0, -180.0),
        dimensions: UVec2::new(width, height),
        cell_size: Vec2::splat(cell_size),
    };

    GridOpacityMap::from_fn(spec, |cell| {
        support::DEMO_GRID[cell.y as usize].as_bytes()[cell.x as usize] == b'#'
    })
}

fn setup(
    mut commands: Commands,
    grid: Res<GridOpacityMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((Name::new("Lab Camera"), Camera2d));
    spawn_grid_tiles(&mut commands, &grid);

    let grid_viewer_position = support::grid_world_position(&grid.spec, GRID_PATH[0], 4.0);
    let grid_viewer = commands
        .spawn((
        Name::new("Grid Viewer"),
        LabGridViewer,
        GridFov::new(4),
        Sprite {
            color: Color::srgb(0.93, 0.80, 0.28),
            custom_size: Some(Vec2::splat(grid.spec.cell_size.x * 0.58)),
            ..default()
        },
        Transform::from_translation(grid_viewer_position),
        GlobalTransform::from_translation(grid_viewer_position),
    ))
        .id();

    commands.spawn((
        Name::new("Cone Arena"),
        Mesh2d(meshes.add(Rectangle::new(620.0, 520.0))),
        MeshMaterial2d(color_materials.add(Color::srgb(0.09, 0.10, 0.13))),
        Transform::from_xyz(365.0, 0.0, -2.0),
    ));

    let guard = commands
        .spawn((
        Name::new("Guard"),
        LabGuard,
        SpatialFov::cone_2d(420.0, 0.56)
            .with_near_override(42.0)
            .with_awareness(SpatialAwarenessConfig {
                alert_threshold: 0.8,
                focused_half_angle_radians: 0.24,
                ..default()
            }),
        Sprite {
            color: Color::srgb(0.97, 0.58, 0.22),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_xyz(180.0, 0.0, 5.0),
        GlobalTransform::from_xyz(180.0, 0.0, 5.0),
    ))
        .id();

    commands.spawn((
        Name::new("Arena Occluder"),
        FovOccluder::new(OccluderShape::Rect2d {
            half_extents: Vec2::new(24.0, 110.0),
        }),
        Sprite {
            color: Color::srgb(0.24, 0.26, 0.30),
            custom_size: Some(Vec2::new(48.0, 220.0)),
            ..default()
        },
        Transform::from_xyz(420.0, 30.0, 2.0),
        GlobalTransform::from_xyz(420.0, 30.0, 2.0),
    ));

    let mut awareness_target = None;
    for (name, position) in [
        (AWARENESS_TARGET_NAME, Vec3::new(340.0, 0.0, 3.0)),
        (FRONT_TARGET_NAME, Vec3::new(570.0, -150.0, 3.0)),
        ("Off-Angle Target", Vec3::new(320.0, 210.0, 3.0)),
        (HIDDEN_TARGET_NAME, Vec3::new(565.0, 120.0, 3.0)),
    ] {
        let mut entity = commands.spawn((
            Name::new(name),
            TargetVisual,
            FovTarget::default(),
            Sprite {
                color: Color::srgb(0.58, 0.62, 0.68),
                custom_size: Some(Vec2::splat(26.0)),
                ..default()
            },
            Transform::from_translation(position),
            GlobalTransform::from_translation(position),
        ));

        if name == AWARENESS_TARGET_NAME {
            entity.insert(FovPerceptionModifiers {
                light_exposure: 1.4,
                awareness_gain_multiplier: 1.6,
                ..default()
            });
        }

        let entity = entity.id();
        if name == AWARENESS_TARGET_NAME {
            awareness_target = Some(entity);
        }
    }

    commands.insert_resource(LabEntities {
        grid_viewer,
        guard,
        awareness_target: awareness_target.expect("awareness target should exist"),
    });

    commands.spawn((
        Name::new("Lab Overlay"),
        LabOverlay,
        Text::new(String::new()),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            ..default()
        },
    ));
}

fn animate_grid_viewer(
    time: Res<Time>,
    grid: Res<GridOpacityMap>,
    control: Res<LabControl>,
    mut viewer: Single<(&mut Transform, &mut GlobalTransform), With<LabGridViewer>>,
) {
    if control.pause_motion {
        return;
    }

    let position = support::sample_path(
        &grid.spec,
        GRID_PATH,
        time.elapsed_secs(),
        control.grid_speed,
        4.0,
    );
    viewer.0.translation = position;
    *viewer.1 = GlobalTransform::from_translation(position);
}

fn animate_guard(
    time: Res<Time>,
    control: Res<LabControl>,
    mut guard: Single<(&mut Transform, &mut GlobalTransform), With<LabGuard>>,
) {
    let angle = if control.pause_motion {
        control.guard_angle
    } else {
        (time.elapsed_secs() * control.guard_sweep_speed).sin() * 0.85
    };

    guard.0.rotation = Quat::from_rotation_z(angle);
    *guard.1 = GlobalTransform::from(*guard.0.as_ref());
}

fn sync_lab_settings(
    control: Res<LabControl>,
    mut grid_viewer: Single<&mut GridFov, With<LabGridViewer>>,
    mut guard: Single<&mut SpatialFov, With<LabGuard>>,
) {
    if !control.is_changed() {
        return;
    }

    grid_viewer.config.radius = control.grid_radius.max(0);

    guard.shape = saddle_ai_fov::SpatialShape::Cone {
        range: control.guard_range.max(0.0),
        half_angle_radians: control.guard_half_angle.max(0.0),
    };
    guard.near_override = control.near_override.max(0.0);
    guard.awareness.alert_threshold = control.alert_threshold.clamp(0.0, guard.awareness.max_awareness);
    guard.awareness.focused_half_angle_radians = control.focused_half_angle.max(0.0);
}

fn tint_grid(
    grid: Res<GridOpacityMap>,
    viewer: Single<&GridFovState, With<LabGridViewer>>,
    mut tiles: Query<(&GridCellSprite, &mut Sprite)>,
) {
    apply_grid_visibility_colors(&grid, &viewer.visible_now, &viewer.explored, &mut tiles);
}

fn tint_targets(
    guard: Single<&SpatialFovState, With<LabGuard>>,
    mut targets: Query<(Entity, &mut Sprite), With<TargetVisual>>,
) {
    for (entity, mut sprite) in &mut targets {
        sprite.color = if guard.visible_now.contains(&entity) {
            Color::srgb(0.20, 0.88, 0.52)
        } else if guard.remembered.contains(&entity) {
            Color::srgb(0.36, 0.44, 0.40)
        } else {
            Color::srgb(0.58, 0.62, 0.68)
        };
    }
}

fn update_diagnostics(
    grid_viewer: Single<&GridFovState, With<LabGridViewer>>,
    guard: Single<&SpatialFovState, With<LabGuard>>,
    target_names: Query<(Entity, &Name), With<TargetVisual>>,
    mut diagnostics: ResMut<LabDiagnostics>,
) {
    diagnostics.grid_visible_cells = grid_viewer.visible_now.len();
    diagnostics.grid_explored_cells = grid_viewer.explored.len();
    diagnostics.memory_sample_visible = grid_viewer.visible_now.contains(&MEMORY_SAMPLE_CELL);
    diagnostics.memory_sample_explored = grid_viewer.explored.contains(&MEMORY_SAMPLE_CELL);
    diagnostics.guard_visible_targets = guard.visible_now.len();
    diagnostics.remembered_targets = guard.remembered.len();
    diagnostics.front_target_visible = false;
    diagnostics.front_target_awareness = 0.0;
    diagnostics.awareness_target_visible = false;
    diagnostics.awareness_target_awareness = 0.0;
    diagnostics.hidden_target_visible = false;

    for (entity, name) in &target_names {
        if name.as_str() == AWARENESS_TARGET_NAME {
            diagnostics.awareness_target_visible = guard.visible_now.contains(&entity);
            diagnostics.awareness_target_awareness =
                guard.awareness_of(entity).map_or(0.0, |entry| entry.awareness);
        }
        if name.as_str() == FRONT_TARGET_NAME {
            diagnostics.front_target_visible = guard.visible_now.contains(&entity);
            diagnostics.front_target_awareness =
                guard.awareness_of(entity).map_or(0.0, |entry| entry.awareness);
        }
        if name.as_str() == HIDDEN_TARGET_NAME {
            diagnostics.hidden_target_visible = guard.visible_now.contains(&entity);
        }
    }
}

fn update_overlay(
    diagnostics: Res<LabDiagnostics>,
    mut overlay: Single<&mut Text, With<LabOverlay>>,
) {
    overlay.0 = format!(
        "fov lab\n\
         left: recursive grid FOV + exploration memory\n\
         right: cone visibility + generic occluder primitives\n\
         visible cells: {}\n\
         explored cells: {}\n\
         guard targets: {}\n\
         remembered targets: {}",
        diagnostics.grid_visible_cells,
        diagnostics.grid_explored_cells,
        diagnostics.guard_visible_targets,
        diagnostics.remembered_targets,
    );
}

pub fn set_pause_motion(world: &mut World, paused: bool) {
    world.resource_mut::<LabControl>().pause_motion = paused;
}

pub fn set_grid_viewer_cell(world: &mut World, cell: IVec2) {
    let position =
        support::grid_world_position(&world.resource::<GridOpacityMap>().spec, cell, 4.0);
    let entity = world.resource::<LabEntities>().grid_viewer;
    let mut entity = world.entity_mut(entity);
    entity.insert((
        FovDirty,
        Transform::from_translation(position),
        GlobalTransform::from_translation(position),
    ));
}

pub fn set_guard_angle(world: &mut World, angle: f32) {
    world.resource_mut::<LabControl>().guard_angle = angle;
    let entity = world.resource::<LabEntities>().guard;

    let translation = world
        .get::<Transform>(entity)
        .expect("guard transform should exist")
        .translation;
    let mut entity = world.entity_mut(entity);
    entity.insert((
        FovDirty,
        Transform::from_translation(translation).with_rotation(Quat::from_rotation_z(angle)),
        GlobalTransform::from_translation(translation)
            * GlobalTransform::from_rotation(Quat::from_rotation_z(angle)),
    ));
}

pub fn awareness_target_awareness(world: &World) -> Option<(saddle_ai_fov::AwarenessLevel, f32)> {
    let entities = world.get_resource::<LabEntities>()?;
    let state = world.get::<SpatialFovState>(entities.guard)?;
    let entry = state.awareness_of(entities.awareness_target)?;
    Some((entry.level, entry.awareness))
}
