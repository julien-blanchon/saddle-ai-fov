use bevy::prelude::*;
use saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovTarget, OccluderShape, SpatialFov, SpatialFovState,
};

#[derive(Component)]
struct Guard;

#[derive(Component)]
struct TargetVisual;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
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
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            animate_guard.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            update_target_colors.after(saddle_ai_fov::FovSystems::Recompute),
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
    mut guard: Single<(&mut Transform, &mut GlobalTransform), With<Guard>>,
) {
    let angle = (time.elapsed_secs() * 0.55).sin() * 0.9;
    let rotation = Quat::from_rotation_z(angle);
    guard.0.rotation = rotation;
    *guard.1 = GlobalTransform::from(*guard.0.as_ref());
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
