use bevy::prelude::*;
use saddle_ai_saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovTarget, OccluderShape, SpatialFov, SpatialFovState,
};

#[derive(Component)]
struct Sentry;

#[derive(Component)]
struct TargetMesh;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.03, 0.035)))
        .insert_resource(FovDebugSettings {
            enabled: true,
            draw_grid_cells: false,
            draw_view_shapes: true,
            draw_occlusion_rays: true,
            max_grid_cells_per_viewer: 0,
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov cone_3d".into(),
                resolution: (1280, 860).into(),
                ..default()
            }),
            ..default()
        }))
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, animate_sentry.before(saddle_ai_fov::FovSystems::MarkDirty))
        .add_systems(Update, tint_targets.after(saddle_ai_fov::FovSystems::Recompute))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Scene Camera"),
        Camera3d::default(),
        Transform::from_xyz(0.0, 180.0, 420.0).looking_at(Vec3::new(0.0, 40.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            illuminance: 22_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.55, -0.55, 0.0)),
    ));
    commands.spawn((
        Name::new("Ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(900.0, 900.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.12, 0.13),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));

    commands.spawn((
        Name::new("Sentry"),
        Sentry,
        SpatialFov::cone_3d(280.0, 0.52)
            .with_local_forward(Vec3::Z)
            .with_local_origin(Vec3::new(0.0, 18.0, 0.0))
            .with_near_override(46.0),
        Mesh3d(meshes.add(Capsule3d::new(16.0, 42.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.54, 0.26),
            ..default()
        })),
        Transform::from_xyz(-180.0, 24.0, 0.0),
        GlobalTransform::from_xyz(-180.0, 24.0, 0.0),
    ));

    commands.spawn((
        Name::new("Occluder Box"),
        FovOccluder::new(OccluderShape::Box {
            half_extents: Vec3::new(26.0, 70.0, 60.0),
        }),
        Mesh3d(meshes.add(Cuboid::new(52.0, 140.0, 120.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.28, 0.30, 0.33),
            ..default()
        })),
        Transform::from_xyz(20.0, 70.0, 0.0),
        GlobalTransform::from_xyz(20.0, 70.0, 0.0),
    ));

    for (name, position) in [
        ("Front Target", Vec3::new(120.0, 20.0, 60.0)),
        ("Rear Target", Vec3::new(-40.0, 20.0, 180.0)),
        ("Occluded Target", Vec3::new(220.0, 20.0, 0.0)),
    ] {
        commands.spawn((
            Name::new(name),
            TargetMesh,
            FovTarget::default().with_sample_points(vec![
                Vec3::ZERO,
                Vec3::new(0.0, 18.0, 0.0),
                Vec3::new(0.0, 36.0, 0.0),
            ]),
            Mesh3d(meshes.add(Cuboid::new(30.0, 36.0, 30.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.54, 0.58, 0.62),
                ..default()
            })),
            Transform::from_translation(position),
            GlobalTransform::from_translation(position),
        ));
    }
}

fn animate_sentry(
    time: Res<Time>,
    mut sentry: Single<(&mut Transform, &mut GlobalTransform), With<Sentry>>,
) {
    let angle = (time.elapsed_secs() * 0.35).sin() * 0.65 - 0.4;
    sentry.0.rotation = Quat::from_rotation_y(angle);
    *sentry.1 = GlobalTransform::from(*sentry.0.as_ref());
}

fn tint_targets(
    sentry: Single<&SpatialFovState, With<Sentry>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    targets: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<TargetMesh>>,
) {
    for (entity, material) in &targets {
        let Some(material) = materials.get_mut(material) else {
            continue;
        };
        material.base_color = if sentry.visible_now.contains(&entity) {
            Color::srgb(0.24, 0.90, 0.50)
        } else if sentry.remembered.contains(&entity) {
            Color::srgb(0.36, 0.44, 0.40)
        } else {
            Color::srgb(0.54, 0.58, 0.62)
        };
    }
}
