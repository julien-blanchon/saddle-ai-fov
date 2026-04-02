#[cfg(feature = "e2e")]
mod e2e;
#[cfg(feature = "e2e")]
mod scenarios;

use saddle_ai_fov_example_support as support;

use bevy::prelude::*;
#[cfg(all(feature = "dev", not(target_arch = "wasm32")))]
use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
#[cfg(all(feature = "dev", not(target_arch = "wasm32")))]
use bevy_brp_extras::BrpExtrasPlugin;
use saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovTarget, GridFov, GridFovState, GridOpacityMap,
    OccluderShape, SpatialFov, SpatialFovState,
};
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
pub const HIDDEN_TARGET_NAME: &str = "Hidden Target";

#[derive(Component)]
struct LabGridViewer;

#[derive(Component)]
struct LabGuard;

#[derive(Component)]
struct LabOverlay;

#[derive(Component)]
struct TargetVisual;

#[derive(Resource, Debug, Clone, Copy)]
pub struct LabControl {
    pub pause_motion: bool,
    pub grid_progress: f32,
    pub guard_angle: f32,
}

impl Default for LabControl {
    fn default() -> Self {
        Self {
            pause_motion: false,
            grid_progress: 0.0,
            guard_angle: 0.0,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct LabDiagnostics {
    pub grid_visible_cells: usize,
    pub grid_explored_cells: usize,
    pub memory_sample_visible: bool,
    pub memory_sample_explored: bool,
    pub guard_visible_targets: usize,
    pub remembered_targets: usize,
    pub front_target_visible: bool,
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
    #[cfg(all(feature = "dev", not(target_arch = "wasm32")))]
    app.add_plugins(RemotePlugin::default());
    #[cfg(all(feature = "dev", not(target_arch = "wasm32")))]
    app.add_plugins(BrpExtrasPlugin::with_http_plugin(
        RemoteHttpPlugin::default(),
    ));
    #[cfg(feature = "e2e")]
    app.add_plugins(e2e::FovLabE2EPlugin);
    app.add_plugins(FovPlugin::default());
    app.add_systems(Startup, setup);
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
    commands.spawn((
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
    ));

    commands.spawn((
        Name::new("Cone Arena"),
        Mesh2d(meshes.add(Rectangle::new(620.0, 520.0))),
        MeshMaterial2d(color_materials.add(Color::srgb(0.09, 0.10, 0.13))),
        Transform::from_xyz(365.0, 0.0, -2.0),
    ));

    commands.spawn((
        Name::new("Guard"),
        LabGuard,
        SpatialFov::cone_2d(420.0, 0.56).with_near_override(42.0),
        Sprite {
            color: Color::srgb(0.97, 0.58, 0.22),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_xyz(180.0, 0.0, 5.0),
        GlobalTransform::from_xyz(180.0, 0.0, 5.0),
    ));

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

    for (name, position) in [
        (FRONT_TARGET_NAME, Vec3::new(570.0, -150.0, 3.0)),
        ("Off-Angle Target", Vec3::new(320.0, 210.0, 3.0)),
        (HIDDEN_TARGET_NAME, Vec3::new(565.0, 120.0, 3.0)),
    ] {
        commands.spawn((
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
    }

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

    let position =
        support::sample_path(&grid.spec, GRID_PATH, time.elapsed_secs(), GRID_SPEED, 4.0);
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
        (time.elapsed_secs() * 0.6).sin() * 0.85
    };

    guard.0.rotation = Quat::from_rotation_z(angle);
    *guard.1 = GlobalTransform::from(*guard.0.as_ref());
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
    diagnostics.hidden_target_visible = false;

    for (entity, name) in &target_names {
        if name.as_str() == FRONT_TARGET_NAME {
            diagnostics.front_target_visible = guard.visible_now.contains(&entity);
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
    let entity = world
        .query_filtered::<Entity, With<LabGridViewer>>()
        .single(world)
        .expect("grid viewer should exist");
    let mut entity = world.entity_mut(entity);
    entity.insert((
        Transform::from_translation(position),
        GlobalTransform::from_translation(position),
    ));
}

pub fn set_guard_angle(world: &mut World, angle: f32) {
    world.resource_mut::<LabControl>().guard_angle = angle;
    let entity = world
        .query_filtered::<Entity, With<LabGuard>>()
        .single(world)
        .expect("guard should exist");

    let translation = world
        .get::<Transform>(entity)
        .expect("guard transform should exist")
        .translation;
    let mut entity = world.entity_mut(entity);
    entity.insert((
        Transform::from_translation(translation).with_rotation(Quat::from_rotation_z(angle)),
        GlobalTransform::from_translation(translation)
            * GlobalTransform::from_rotation(Quat::from_rotation_z(angle)),
    ));
}
