use bevy::prelude::*;
use saddle_ai_fov::{
    FovDebugSettings, FovPlugin, FovTarget, SpatialFov, SpatialFovState, VisibilityLayer,
    VisibilityLayerMask,
};
use saddle_pane::prelude::*;

const LAYER_RED: VisibilityLayer = VisibilityLayer(0);
const LAYER_BLUE: VisibilityLayer = VisibilityLayer(1);

#[derive(Component)]
struct RedViewer;

#[derive(Component)]
struct BlueViewer;

#[derive(Component)]
struct TargetVisual {
    base_color: Color,
}

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "Visibility Layers", position = "top-right")]
struct LayersPane {
    #[pane(slider, min = 120.0, max = 400.0, step = 5.0)]
    range: f32,
    #[pane(slider, min = 0.1, max = 0.8, step = 0.02)]
    orbit_speed: f32,
    #[pane(monitor)]
    red_sees: usize,
    #[pane(monitor)]
    blue_sees: usize,
}

impl Default for LayersPane {
    fn default() -> Self {
        Self {
            range: 280.0,
            orbit_speed: 0.25,
            red_sees: 0,
            blue_sees: 0,
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
        .insert_resource(LayersPane::default())
        .insert_resource(FovDebugSettings {
            enabled: true,
            draw_grid_cells: false,
            draw_view_shapes: true,
            draw_filled_shapes: true,
            draw_occlusion_rays: true,
            max_grid_cells_per_viewer: 0,
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov layers".into(),
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
        .register_pane::<LayersPane>()
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
            (update_target_colors, update_pane).after(saddle_ai_fov::FovSystems::Recompute),
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
        Mesh2d(meshes.add(Rectangle::new(1000.0, 660.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.08, 0.09, 0.11))),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));

    // Red viewer — sees only red-layer targets
    commands.spawn((
        Name::new("Red Viewer"),
        RedViewer,
        SpatialFov::radius(280.0).with_layers(VisibilityLayerMask::from_layer(LAYER_RED)),
        Sprite {
            color: Color::srgb(0.92, 0.30, 0.25),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_xyz(-200.0, 0.0, 4.0),
        GlobalTransform::from_xyz(-200.0, 0.0, 4.0),
    ));

    // Blue viewer — sees only blue-layer targets
    commands.spawn((
        Name::new("Blue Viewer"),
        BlueViewer,
        SpatialFov::radius(280.0).with_layers(VisibilityLayerMask::from_layer(LAYER_BLUE)),
        Sprite {
            color: Color::srgb(0.25, 0.50, 0.92),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_xyz(200.0, 0.0, 4.0),
        GlobalTransform::from_xyz(200.0, 0.0, 4.0),
    ));

    // Red targets (layer 0) — only visible to red viewer
    let red_color = Color::srgb(0.80, 0.35, 0.30);
    for (i, pos) in [
        Vec3::new(-60.0, 180.0, 3.0),
        Vec3::new(100.0, -120.0, 3.0),
        Vec3::new(-160.0, -160.0, 3.0),
    ]
    .iter()
    .enumerate()
    {
        commands.spawn((
            Name::new(format!("Red Target {i}")),
            TargetVisual { base_color: red_color },
            FovTarget::default().with_layers(VisibilityLayerMask::from_layer(LAYER_RED)),
            Sprite {
                color: red_color,
                custom_size: Some(Vec2::splat(22.0)),
                ..default()
            },
            Transform::from_translation(*pos),
            GlobalTransform::from_translation(*pos),
        ));
    }

    // Blue targets (layer 1) — only visible to blue viewer
    let blue_color = Color::srgb(0.30, 0.50, 0.85);
    for (i, pos) in [
        Vec3::new(60.0, 160.0, 3.0),
        Vec3::new(-120.0, -80.0, 3.0),
        Vec3::new(180.0, -180.0, 3.0),
    ]
    .iter()
    .enumerate()
    {
        commands.spawn((
            Name::new(format!("Blue Target {i}")),
            TargetVisual { base_color: blue_color },
            FovTarget::default().with_layers(VisibilityLayerMask::from_layer(LAYER_BLUE)),
            Sprite {
                color: blue_color,
                custom_size: Some(Vec2::splat(22.0)),
                ..default()
            },
            Transform::from_translation(*pos),
            GlobalTransform::from_translation(*pos),
        ));
    }

    // Shared targets (both layers) — visible to both viewers
    let shared_color = Color::srgb(0.70, 0.55, 0.80);
    for (i, pos) in [
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::new(0.0, -220.0, 3.0),
    ]
    .iter()
    .enumerate()
    {
        commands.spawn((
            Name::new(format!("Shared Target {i}")),
            TargetVisual { base_color: shared_color },
            FovTarget::default().with_layers(
                VisibilityLayerMask::from_layer(LAYER_RED)
                    .union(VisibilityLayerMask::from_layer(LAYER_BLUE)),
            ),
            Sprite {
                color: shared_color,
                custom_size: Some(Vec2::splat(26.0)),
                ..default()
            },
            Transform::from_translation(*pos),
            GlobalTransform::from_translation(*pos),
        ));
    }

    commands.spawn((
        Name::new("Example Label"),
        Text::new(
            "layers: red viewer sees red+shared targets, blue sees blue+shared — same world, filtered perception",
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
    pane: Res<LayersPane>,
    mut targets: Query<(&mut Transform, &mut GlobalTransform), With<TargetVisual>>,
) {
    let elapsed = time.elapsed_secs() * pane.orbit_speed;
    for (i, (mut transform, mut global)) in targets.iter_mut().enumerate() {
        let base_angle = std::f32::consts::TAU * i as f32 / 8.0;
        let angle = base_angle + elapsed;
        let radius = 120.0 + 90.0 * ((i as f32 * 0.7) + 0.5).sin();
        let pos = Vec3::new(angle.cos() * radius, angle.sin() * radius, 3.0);
        transform.translation = pos;
        *global = GlobalTransform::from_translation(pos);
    }
}

fn sync_controls(
    pane: Res<LayersPane>,
    mut red: Single<&mut SpatialFov, (With<RedViewer>, Without<BlueViewer>)>,
    mut blue: Single<&mut SpatialFov, (With<BlueViewer>, Without<RedViewer>)>,
) {
    if !pane.is_changed() {
        return;
    }
    red.shape = saddle_ai_fov::SpatialShape::Radius {
        range: pane.range.max(0.0),
    };
    blue.shape = saddle_ai_fov::SpatialShape::Radius {
        range: pane.range.max(0.0),
    };
}

fn update_target_colors(
    red_state: Single<&SpatialFovState, With<RedViewer>>,
    blue_state: Single<&SpatialFovState, With<BlueViewer>>,
    mut targets: Query<(Entity, &TargetVisual, &mut Sprite)>,
) {
    for (entity, visual, mut sprite) in &mut targets {
        let seen_by_red = red_state.visible_now.contains(&entity);
        let seen_by_blue = blue_state.visible_now.contains(&entity);
        sprite.color = if seen_by_red || seen_by_blue {
            Color::srgb(0.20, 0.88, 0.48)
        } else {
            visual.base_color
        };
    }
}

fn update_pane(
    red: Single<&SpatialFovState, With<RedViewer>>,
    blue: Single<&SpatialFovState, With<BlueViewer>>,
    mut pane: ResMut<LayersPane>,
) {
    pane.red_sees = red.visible_now.len();
    pane.blue_sees = blue.visible_now.len();
}
