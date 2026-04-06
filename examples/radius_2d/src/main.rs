use bevy::prelude::*;
use saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovTarget, OccluderShape, SpatialFov, SpatialFovState,
};
use saddle_pane::prelude::*;

#[derive(Component)]
struct Beacon;

#[derive(Component)]
struct TargetVisual;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "2D Radius FOV", position = "top-right")]
struct Radius2dPane {
    #[pane(slider, min = 80.0, max = 400.0, step = 5.0)]
    range: f32,
    #[pane(slider, min = 0.0, max = 64.0, step = 2.0)]
    near_override: f32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.02)]
    orbit_speed: f32,
    #[pane(monitor)]
    visible_targets: usize,
}

impl Default for Radius2dPane {
    fn default() -> Self {
        Self {
            range: 260.0,
            near_override: 0.0,
            orbit_speed: 0.35,
            visible_targets: 0,
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
        .insert_resource(Radius2dPane::default())
        .insert_resource(FovDebugSettings {
            enabled: true,
            draw_grid_cells: false,
            draw_view_shapes: true,
            draw_filled_shapes: true,
            draw_occlusion_rays: true,
            draw_blocked_rays: true,
            draw_occluder_shapes: true,
            max_grid_cells_per_viewer: 0,
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov radius_2d".into(),
                resolution: (1280, 840).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            bevy_flair::FlairPlugin,
            bevy_input_focus::InputDispatchPlugin,
            bevy_ui_widgets::UiWidgetsPlugins,
            bevy_input_focus::tab_navigation::TabNavigationPlugin,
            PanePlugin,
        ))
        .register_pane::<Radius2dPane>()
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_controls.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            orbit_targets.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            update_target_colors.after(saddle_ai_fov::FovSystems::Recompute),
        )
        .add_systems(
            Update,
            update_pane.after(saddle_ai_fov::FovSystems::Recompute),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((Name::new("Example Camera"), Camera2d));

    commands.spawn((
        Name::new("Arena"),
        Mesh2d(meshes.add(Rectangle::new(960.0, 640.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.10, 0.11, 0.14))),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));

    commands.spawn((
        Name::new("Beacon"),
        Beacon,
        SpatialFov::radius(260.0),
        Sprite {
            color: Color::srgb(0.30, 0.84, 0.95),
            custom_size: Some(Vec2::splat(30.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 4.0),
        GlobalTransform::from_xyz(0.0, 0.0, 4.0),
    ));

    // Two occluder walls
    for (name, pos, size) in [
        (
            "Wall Left",
            Vec3::new(-100.0, 30.0, 2.0),
            Vec2::new(24.0, 180.0),
        ),
        (
            "Wall Right",
            Vec3::new(130.0, -40.0, 2.0),
            Vec2::new(24.0, 160.0),
        ),
    ] {
        commands.spawn((
            Name::new(name),
            FovOccluder::new(OccluderShape::Rect2d {
                half_extents: size * 0.5,
            }),
            Sprite {
                color: Color::srgb(0.24, 0.25, 0.29),
                custom_size: Some(size),
                ..default()
            },
            Transform::from_translation(pos),
            GlobalTransform::from_translation(pos),
        ));
    }

    // Orbiting targets at varying distances
    let target_positions = [
        Vec3::new(180.0, 0.0, 3.0),
        Vec3::new(-160.0, 120.0, 3.0),
        Vec3::new(60.0, -200.0, 3.0),
        Vec3::new(-80.0, -140.0, 3.0),
        Vec3::new(240.0, 160.0, 3.0),
        Vec3::new(0.0, 220.0, 3.0),
    ];
    for (i, position) in target_positions.iter().enumerate() {
        commands.spawn((
            Name::new(format!("Target {}", i)),
            TargetVisual,
            FovTarget::default(),
            Sprite {
                color: Color::srgb(0.55, 0.60, 0.65),
                custom_size: Some(Vec2::splat(24.0)),
                ..default()
            },
            Transform::from_translation(*position),
            GlobalTransform::from_translation(*position),
        ));
    }

    commands.spawn((
        Name::new("Example Label"),
        Text::new(
            "radius_2d: omnidirectional detection radius with occluder primitives.\nControls: use the top-right pane to tune range, near override, and orbit speed.",
        ),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            ..default()
        },
    ));
}

fn orbit_targets(
    time: Res<Time>,
    pane: Res<Radius2dPane>,
    mut targets: Query<(&mut Transform, &mut GlobalTransform), With<TargetVisual>>,
) {
    let elapsed = time.elapsed_secs() * pane.orbit_speed;
    for (i, (mut transform, mut global)) in targets.iter_mut().enumerate() {
        let base_angle = std::f32::consts::TAU * i as f32 / 6.0;
        let angle = base_angle + elapsed;
        let radius = 140.0 + 80.0 * (i as f32 * 1.3).sin();
        let pos = Vec3::new(angle.cos() * radius, angle.sin() * radius, 3.0);
        transform.translation = pos;
        *global = GlobalTransform::from_translation(pos);
    }
}

fn sync_controls(pane: Res<Radius2dPane>, mut beacon: Single<&mut SpatialFov, With<Beacon>>) {
    if !pane.is_changed() {
        return;
    }
    beacon.shape = saddle_ai_fov::SpatialShape::Radius {
        range: pane.range.max(0.0),
    };
    beacon.near_override = pane.near_override.max(0.0);
}

fn update_target_colors(
    beacon: Single<&SpatialFovState, With<Beacon>>,
    mut targets: Query<(Entity, &mut Sprite), With<TargetVisual>>,
) {
    for (entity, mut sprite) in &mut targets {
        sprite.color = if beacon.visible_now.contains(&entity) {
            Color::srgb(0.18, 0.86, 0.52)
        } else {
            Color::srgb(0.55, 0.60, 0.65)
        };
    }
}

fn update_pane(beacon: Single<&SpatialFovState, With<Beacon>>, mut pane: ResMut<Radius2dPane>) {
    pane.visible_targets = beacon.visible_now.len();
}
