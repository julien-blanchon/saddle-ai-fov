use bevy::prelude::*;
use saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovTarget, OccluderShape, SpatialFov, SpatialFovState,
};
use saddle_pane::prelude::*;

#[derive(Component)]
struct Camera;

#[derive(Component)]
struct TargetVisual;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "2D Rect FOV", position = "top-right")]
struct Rect2dPane {
    #[pane]
    pause_motion: bool,
    #[pane(slider, min = 100.0, max = 500.0, step = 5.0)]
    depth: f32,
    #[pane(slider, min = 30.0, max = 250.0, step = 5.0)]
    half_width: f32,
    #[pane(slider, min = 0.0, max = 80.0, step = 2.0)]
    near_override: f32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.02)]
    sweep_speed: f32,
    #[pane(monitor)]
    visible_targets: usize,
    #[pane(monitor)]
    remembered_targets: usize,
}

impl Default for Rect2dPane {
    fn default() -> Self {
        Self {
            pause_motion: false,
            depth: 340.0,
            half_width: 120.0,
            near_override: 40.0,
            sweep_speed: 0.40,
            visible_targets: 0,
            remembered_targets: 0,
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
        .insert_resource(Rect2dPane::default())
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
                title: "fov rect_2d".into(),
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
        .register_pane::<Rect2dPane>()
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_controls.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_camera.before(saddle_ai_fov::FovSystems::MarkDirty),
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
        Mesh2d(meshes.add(Rectangle::new(1100.0, 700.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.10, 0.11, 0.14))),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));

    commands.spawn((
        Name::new("Security Camera"),
        Camera,
        SpatialFov::rect_2d(340.0, 120.0).with_near_override(40.0),
        Sprite {
            color: Color::srgb(0.20, 0.70, 0.95),
            custom_size: Some(Vec2::new(28.0, 20.0)),
            ..default()
        },
        Transform::from_xyz(-340.0, 0.0, 4.0),
        GlobalTransform::from_xyz(-340.0, 0.0, 4.0),
    ));

    // Occluder walls
    for (name, pos, size) in [
        (
            "Wall A",
            Vec3::new(-60.0, 60.0, 2.0),
            Vec2::new(30.0, 200.0),
        ),
        (
            "Wall B",
            Vec3::new(140.0, -40.0, 2.0),
            Vec2::new(30.0, 160.0),
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

    // Scattered targets
    let target_positions = [
        Vec3::new(60.0, 160.0, 3.0),
        Vec3::new(200.0, 0.0, 3.0),
        Vec3::new(-120.0, -140.0, 3.0),
        Vec3::new(280.0, 100.0, 3.0),
        Vec3::new(20.0, -80.0, 3.0),
        Vec3::new(340.0, -140.0, 3.0),
    ];
    for (i, position) in target_positions.iter().enumerate() {
        commands.spawn((
            Name::new(format!("Target {i}")),
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
            "rect_2d: rectangular field of view for cameras, sensors, and corridor detection.\nControls: use the top-right pane to pause the sweep and tune depth, width, and near override.",
        ),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            ..default()
        },
    ));
}

fn animate_camera(
    time: Res<Time>,
    pane: Res<Rect2dPane>,
    mut camera: Single<(&mut Transform, &mut GlobalTransform), With<Camera>>,
) {
    if pane.pause_motion {
        return;
    }

    let angle = (time.elapsed_secs() * pane.sweep_speed).sin() * 0.6;
    let rotation = Quat::from_rotation_z(angle);
    camera.0.rotation = rotation;
    *camera.1 = GlobalTransform::from(*camera.0.as_ref());
}

fn sync_controls(pane: Res<Rect2dPane>, mut camera: Single<&mut SpatialFov, With<Camera>>) {
    if !pane.is_changed() {
        return;
    }

    camera.shape = saddle_ai_fov::SpatialShape::Rect {
        depth: pane.depth.max(0.0),
        half_width: pane.half_width.max(0.0),
        half_height: 0.0,
    };
    camera.near_override = pane.near_override.max(0.0);
}

fn update_target_colors(
    camera: Single<&SpatialFovState, With<Camera>>,
    mut targets: Query<(Entity, &mut Sprite), With<TargetVisual>>,
) {
    for (entity, mut sprite) in &mut targets {
        sprite.color = if camera.visible_now.contains(&entity) {
            Color::srgb(0.18, 0.86, 0.52)
        } else if camera.remembered.contains(&entity) {
            Color::srgb(0.38, 0.46, 0.42)
        } else {
            Color::srgb(0.55, 0.60, 0.65)
        };
    }
}

fn update_pane(camera: Single<&SpatialFovState, With<Camera>>, mut pane: ResMut<Rect2dPane>) {
    pane.visible_targets = camera.visible_now.len();
    pane.remembered_targets = camera.remembered.len();
}
