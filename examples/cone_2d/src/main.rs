use bevy::prelude::*;
use saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovTarget, OccluderShape, SpatialFov, SpatialFovState,
};
use saddle_pane::prelude::*;

#[derive(Component)]
struct Guard;

#[derive(Component)]
struct TargetVisual;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "2D Cone FOV", position = "top-right")]
struct Cone2dPane {
    #[pane]
    pause_motion: bool,
    #[pane(slider, min = 120.0, max = 480.0, step = 5.0)]
    range: f32,
    #[pane(slider, min = 0.15, max = 1.2, step = 0.02)]
    half_angle: f32,
    #[pane(slider, min = 0.0, max = 96.0, step = 2.0)]
    near_override: f32,
    #[pane(slider, min = 0.1, max = 1.2, step = 0.02)]
    sweep_speed: f32,
    #[pane(monitor)]
    visible_targets: usize,
    #[pane(monitor)]
    remembered_targets: usize,
}

impl Default for Cone2dPane {
    fn default() -> Self {
        Self {
            pause_motion: false,
            range: 320.0,
            half_angle: 0.62,
            near_override: 48.0,
            sweep_speed: 0.55,
            visible_targets: 0,
            remembered_targets: 0,
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
        .insert_resource(Cone2dPane::default())
        .insert_resource(FovDebugSettings {
            enabled: true,
            draw_grid_cells: false,
            draw_view_shapes: true,
            draw_occlusion_rays: true,
            max_grid_cells_per_viewer: 0,
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov cone_2d".into(),
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
        .register_pane::<Cone2dPane>()
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_controls.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_guard.before(saddle_ai_fov::FovSystems::MarkDirty),
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
        Mesh2d(meshes.add(Rectangle::new(960.0, 620.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.10, 0.11, 0.14))),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));

    commands.spawn((
        Name::new("Guard"),
        Guard,
        SpatialFov::cone_2d(320.0, 0.62).with_near_override(48.0),
        Sprite {
            color: Color::srgb(0.97, 0.62, 0.20),
            custom_size: Some(Vec2::splat(34.0)),
            ..default()
        },
        Transform::from_xyz(-280.0, 0.0, 4.0),
        GlobalTransform::from_xyz(-280.0, 0.0, 4.0),
    ));

    commands.spawn((
        Name::new("Occluder"),
        FovOccluder::new(OccluderShape::Rect2d {
            half_extents: Vec2::new(28.0, 120.0),
        }),
        Sprite {
            color: Color::srgb(0.24, 0.25, 0.29),
            custom_size: Some(Vec2::new(56.0, 240.0)),
            ..default()
        },
        Transform::from_xyz(40.0, 0.0, 2.0),
        GlobalTransform::from_xyz(40.0, 0.0, 2.0),
    ));

    for (name, position) in [
        ("Target Left", Vec3::new(-20.0, 150.0, 3.0)),
        ("Target Center", Vec3::new(240.0, 0.0, 3.0)),
        ("Target Hidden", Vec3::new(240.0, 120.0, 3.0)),
    ] {
        commands.spawn((
            Name::new(name),
            TargetVisual,
            FovTarget::default(),
            Sprite {
                color: Color::srgb(0.55, 0.60, 0.65),
                custom_size: Some(Vec2::splat(28.0)),
                ..default()
            },
            Transform::from_translation(position),
            GlobalTransform::from_translation(position),
        ));
    }

    commands.spawn((
        Name::new("Example Label"),
        Text::new("cone_2d: directional cone + generic occluder primitives + live gizmo rays"),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            ..default()
        },
    ));
}

fn animate_guard(
    time: Res<Time>,
    pane: Res<Cone2dPane>,
    mut guard: Single<(&mut Transform, &mut GlobalTransform), With<Guard>>,
) {
    if pane.pause_motion {
        return;
    }

    let angle = (time.elapsed_secs() * pane.sweep_speed).sin() * 0.9;
    let rotation = Quat::from_rotation_z(angle);
    guard.0.rotation = rotation;
    *guard.1 = GlobalTransform::from(*guard.0.as_ref());
}

fn sync_controls(
    pane: Res<Cone2dPane>,
    mut guard: Single<&mut SpatialFov, With<Guard>>,
) {
    if !pane.is_changed() {
        return;
    }

    guard.shape = saddle_ai_fov::SpatialShape::Cone {
        range: pane.range.max(0.0),
        half_angle_radians: pane.half_angle.max(0.0),
    };
    guard.near_override = pane.near_override.max(0.0);
}

fn update_target_colors(
    guard: Single<&SpatialFovState, With<Guard>>,
    mut targets: Query<(Entity, &mut Sprite), With<TargetVisual>>,
) {
    for (entity, mut sprite) in &mut targets {
        sprite.color = if guard.visible_now.contains(&entity) {
            Color::srgb(0.18, 0.86, 0.52)
        } else if guard.remembered.contains(&entity) {
            Color::srgb(0.38, 0.46, 0.42)
        } else {
            Color::srgb(0.55, 0.60, 0.65)
        };
    }
}

fn update_pane(guard: Single<&SpatialFovState, With<Guard>>, mut pane: ResMut<Cone2dPane>) {
    pane.visible_targets = guard.visible_now.len();
    pane.remembered_targets = guard.remembered.len();
}
