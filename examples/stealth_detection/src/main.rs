use bevy::prelude::*;
use saddle_ai_fov::{
    AwarenessLevel, FovDebugSettings, FovOccluder, FovPerceptionModifiers, FovPlugin, FovTarget,
    OccluderShape, SpatialAwarenessConfig, SpatialFov, SpatialFovState,
};
use saddle_pane::prelude::*;

#[derive(Component)]
struct Guard;

#[derive(Component)]
struct Infiltrator;

#[derive(Component)]
struct AwarenessBarFill;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "Stealth Detection", position = "top-right")]
struct StealthPane {
    #[pane(slider, min = 180.0, max = 520.0, step = 5.0)]
    guard_range: f32,
    #[pane(slider, min = 0.2, max = 1.2, step = 0.02)]
    guard_half_angle: f32,
    #[pane(slider, min = 0.08, max = 0.7, step = 0.02)]
    focus_half_angle: f32,
    #[pane(slider, min = 0.0, max = 120.0, step = 2.0)]
    near_override: f32,
    #[pane(slider, min = 0.25, max = 0.95, step = 0.01)]
    alert_threshold: f32,
    #[pane(slider, min = 0.15, max = 1.4, step = 0.02)]
    patrol_speed: f32,
    #[pane(slider, min = 0.2, max = 2.0, step = 0.05)]
    spotlight_exposure: f32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.05)]
    shadow_exposure: f32,
    #[pane(slider, min = 0.0, max = 1.0, step = 0.05)]
    grate_noise: f32,
    #[pane(monitor)]
    awareness: f32,
    #[pane(monitor)]
    alerted: bool,
}

impl Default for StealthPane {
    fn default() -> Self {
        Self {
            guard_range: 340.0,
            guard_half_angle: 0.62,
            focus_half_angle: 0.24,
            near_override: 48.0,
            alert_threshold: 0.8,
            patrol_speed: 0.52,
            spotlight_exposure: 1.55,
            shadow_exposure: 0.35,
            grate_noise: 0.65,
            awareness: 0.0,
            alerted: false,
        }
    }
}

const INFILTRATOR_PATH: [Vec3; 6] = [
    Vec3::new(250.0, -160.0, 6.0),
    Vec3::new(180.0, -70.0, 6.0),
    Vec3::new(120.0, 10.0, 6.0),
    Vec3::new(210.0, 105.0, 6.0),
    Vec3::new(320.0, 55.0, 6.0),
    Vec3::new(280.0, -90.0, 6.0),
];

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.03, 0.035, 0.04)))
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
                title: "fov stealth_detection".into(),
                resolution: (1360, 860).into(),
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
        .register_pane::<StealthPane>()
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, sync_pane.before(saddle_ai_fov::FovSystems::MarkDirty))
        .add_systems(
            Update,
            animate_guard.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_infiltrator.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            update_perception_modifiers.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            update_presentation.after(saddle_ai_fov::FovSystems::Recompute),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Stealth Camera"), Camera2d));
    commands.spawn((
        Name::new("Museum Floor"),
        Sprite::from_color(Color::srgb(0.08, 0.09, 0.11), Vec2::new(1120.0, 680.0)),
        Transform::from_xyz(0.0, 0.0, -5.0),
    ));
    commands.spawn((
        Name::new("Gallery Strip"),
        Sprite::from_color(Color::srgb(0.16, 0.12, 0.08), Vec2::new(540.0, 90.0)),
        Transform::from_xyz(170.0, -150.0, -2.0),
    ));
    commands.spawn((
        Name::new("Spotlight Zone"),
        Sprite::from_color(Color::srgba(1.0, 0.92, 0.54, 0.10), Vec2::new(210.0, 210.0)),
        Transform::from_xyz(200.0, 10.0, -1.0),
    ));
    commands.spawn((
        Name::new("Metal Grate"),
        Sprite::from_color(Color::srgba(0.42, 0.48, 0.55, 0.22), Vec2::new(240.0, 56.0)),
        Transform::from_xyz(285.0, 56.0, -1.0),
    ));

    for (name, position, size) in [
        ("Tall Crate", Vec3::new(40.0, 16.0, 2.0), Vec2::new(70.0, 180.0)),
        ("Display Case", Vec3::new(150.0, -110.0, 2.0), Vec2::new(84.0, 126.0)),
        ("Column", Vec3::new(328.0, -8.0, 2.0), Vec2::new(64.0, 160.0)),
    ] {
        commands.spawn((
            Name::new(name),
            FovOccluder::new(OccluderShape::Rect2d {
                half_extents: size * 0.5,
            }),
            Sprite::from_color(Color::srgb(0.26, 0.27, 0.31), size),
            Transform::from_translation(position),
            GlobalTransform::from_translation(position),
        ));
    }

    commands.spawn((
        Name::new("Guard"),
        Guard,
        SpatialFov::cone_2d(340.0, 0.62)
            .with_near_override(48.0)
            .with_awareness(SpatialAwarenessConfig {
                alert_threshold: 0.8,
                focused_half_angle_radians: 0.24,
                ..default()
            }),
        Sprite::from_color(Color::srgb(0.96, 0.57, 0.22), Vec2::splat(34.0)),
        Transform::from_xyz(-300.0, 0.0, 5.0),
        GlobalTransform::from_xyz(-300.0, 0.0, 5.0),
    ));

    commands.spawn((
        Name::new("Infiltrator"),
        Infiltrator,
        FovTarget::default(),
        FovPerceptionModifiers::default(),
        Sprite::from_color(Color::srgb(0.56, 0.84, 0.94), Vec2::new(26.0, 32.0)),
        Transform::from_translation(INFILTRATOR_PATH[0]),
        GlobalTransform::from_translation(INFILTRATOR_PATH[0]),
    ));

    commands.spawn((
        Name::new("Awareness Frame"),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(18.0),
            width: px(320.0),
            height: px(24.0),
            border: UiRect::all(px(2.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.72, 0.72, 0.74)),
        BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.82)),
        children![(
            Name::new("Awareness Fill"),
            AwarenessBarFill,
            Node {
                width: percent(0.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.27, 0.68, 0.92)),
        )],
    ));

    commands.spawn((
        Name::new("Stealth Label"),
        Text::new("stealth_detection: awareness rises faster in bright, noisy spaces"),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(56.0),
            ..default()
        },
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn sync_pane(
    pane: Res<StealthPane>,
    mut guard: Single<&mut SpatialFov, With<Guard>>,
) {
    if !pane.is_changed() {
        return;
    }

    guard.shape = saddle_ai_fov::SpatialShape::Cone {
        range: pane.guard_range,
        half_angle_radians: pane.guard_half_angle,
    };
    guard.near_override = pane.near_override;
    guard.awareness.alert_threshold = pane.alert_threshold;
    guard.awareness.focused_half_angle_radians = pane.focus_half_angle;
}

fn animate_guard(
    time: Res<Time>,
    pane: Res<StealthPane>,
    mut guard: Single<(&mut Transform, &mut GlobalTransform), With<Guard>>,
) {
    let angle = (time.elapsed_secs() * pane.patrol_speed).sin() * 0.9;
    guard.0.rotation = Quat::from_rotation_z(angle);
    *guard.1 = GlobalTransform::from(*guard.0.as_ref());
}

fn animate_infiltrator(
    time: Res<Time>,
    pane: Res<StealthPane>,
    mut infiltrator: Single<(&mut Transform, &mut GlobalTransform), With<Infiltrator>>,
) {
    let progress = time.elapsed_secs() * (0.18 + pane.patrol_speed * 0.18);
    let from = progress.floor() as usize % INFILTRATOR_PATH.len();
    let to = (from + 1) % INFILTRATOR_PATH.len();
    let t = progress.fract();
    let position = INFILTRATOR_PATH[from].lerp(INFILTRATOR_PATH[to], t);
    infiltrator.0.translation = position;
    *infiltrator.1 = GlobalTransform::from_translation(position);
}

fn update_perception_modifiers(
    pane: Res<StealthPane>,
    infiltrator: Single<&Transform, With<Infiltrator>>,
    mut modifiers: Single<&mut FovPerceptionModifiers, With<Infiltrator>>,
) {
    let in_spotlight = infiltrator.translation.truncate().distance(Vec2::new(200.0, 10.0)) < 92.0;
    let on_grate = infiltrator.translation.x > 210.0
        && infiltrator.translation.x < 360.0
        && infiltrator.translation.y > 28.0
        && infiltrator.translation.y < 84.0;

    modifiers.light_exposure = if in_spotlight {
        pane.spotlight_exposure
    } else {
        pane.shadow_exposure
    };
    modifiers.noise_emission = if on_grate { pane.grate_noise } else { 0.0 };
}

fn update_presentation(
    mut pane: ResMut<StealthPane>,
    guard_state: Single<&SpatialFovState, With<Guard>>,
    infiltrator: Single<(Entity, &mut Sprite), With<Infiltrator>>,
    awareness_bar: Single<(&mut Node, &mut BackgroundColor), With<AwarenessBarFill>>,
) {
    let (infiltrator_entity, mut infiltrator_sprite) = infiltrator.into_inner();
    let awareness = guard_state
        .awareness_of(infiltrator_entity)
        .cloned()
        .unwrap_or_else(|| saddle_ai_fov::SpatialAwarenessEntry::new(infiltrator_entity));

    pane.awareness = awareness.awareness;
    pane.alerted = awareness.level == AwarenessLevel::Alert;

    infiltrator_sprite.color = match awareness.level {
        AwarenessLevel::Alert => Color::srgb(0.96, 0.24, 0.18),
        AwarenessLevel::Searching | AwarenessLevel::Suspicious => Color::srgb(0.98, 0.86, 0.36),
        _ => Color::srgb(0.56, 0.84, 0.94),
    };

    let (mut awareness_fill, mut awareness_fill_color) = awareness_bar.into_inner();
    awareness_fill.width = percent((awareness.awareness * 100.0).clamp(0.0, 100.0));
    awareness_fill_color.0 = match awareness.level {
        AwarenessLevel::Alert => Color::srgb(0.90, 0.20, 0.16),
        AwarenessLevel::Searching | AwarenessLevel::Suspicious => Color::srgb(0.94, 0.75, 0.28),
        _ => Color::srgb(0.24, 0.68, 0.92),
    };
}
