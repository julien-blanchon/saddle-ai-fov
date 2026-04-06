use bevy::prelude::*;
use saddle_ai_fov::{
    FovDebugSettings, FovOccluder, FovPlugin, FovStimulusSource, FovTarget, OccluderShape,
    SpatialFov, SpatialFovState, SpatialStimulusConfig,
};
use saddle_pane::prelude::*;

#[derive(Component)]
struct Sensor;

#[derive(Component)]
struct HighlightTarget;

#[derive(Component)]
struct SignalBarFill;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "Perception Kernel", position = "top-right")]
struct KernelPane {
    #[pane(slider, min = 180.0, max = 520.0, step = 5.0)]
    range: f32,
    #[pane(slider, min = 0.2, max = 1.2, step = 0.02)]
    half_angle: f32,
    #[pane(slider, min = 0.08, max = 0.7, step = 0.02)]
    focus_half_angle: f32,
    #[pane(slider, min = 0.0, max = 120.0, step = 2.0)]
    near_override: f32,
    #[pane(slider, min = 0.15, max = 1.4, step = 0.02)]
    sweep_speed: f32,
    #[pane(slider, min = 0.2, max = 2.0, step = 0.05)]
    spotlight_scale: f32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.05)]
    shadow_scale: f32,
    #[pane(slider, min = 0.0, max = 1.0, step = 0.05)]
    indirect_signal: f32,
    #[pane(monitor)]
    highlighted_signal: f32,
    #[pane(monitor)]
    visible_targets: usize,
    #[pane(monitor)]
    remembered_targets: usize,
}

impl Default for KernelPane {
    fn default() -> Self {
        Self {
            range: 340.0,
            half_angle: 0.62,
            focus_half_angle: 0.24,
            near_override: 48.0,
            sweep_speed: 0.52,
            spotlight_scale: 1.55,
            shadow_scale: 0.35,
            indirect_signal: 0.65,
            highlighted_signal: 0.0,
            visible_targets: 0,
            remembered_targets: 0,
        }
    }
}

const TARGET_PATH: [Vec3; 6] = [
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
        .insert_resource(KernelPane::default())
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
                title: "fov perception_kernel".into(),
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
        .register_pane::<KernelPane>()
        .init_gizmo_group::<saddle_ai_fov::FovDebugGizmos>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_pane.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_sensor.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_target.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            update_stimulus_source.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            update_presentation.after(saddle_ai_fov::FovSystems::Recompute),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("Kernel Camera"), Camera2d));
    commands.spawn((
        Name::new("Museum Floor"),
        Sprite::from_color(Color::srgb(0.08, 0.09, 0.11), Vec2::new(1120.0, 680.0)),
        Transform::from_xyz(0.0, 0.0, -5.0),
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
        (
            "Tall Crate",
            Vec3::new(40.0, 16.0, 2.0),
            Vec2::new(70.0, 180.0),
        ),
        (
            "Display Case",
            Vec3::new(150.0, -110.0, 2.0),
            Vec2::new(84.0, 126.0),
        ),
        (
            "Column",
            Vec3::new(328.0, -8.0, 2.0),
            Vec2::new(64.0, 160.0),
        ),
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
        Name::new("Sensor"),
        Sensor,
        SpatialFov::cone_2d(340.0, 0.62)
            .with_near_override(48.0)
            .with_stimulus(SpatialStimulusConfig {
                focused_half_angle_radians: 0.24,
                ..default()
            }),
        Sprite::from_color(Color::srgb(0.96, 0.57, 0.22), Vec2::splat(34.0)),
        Transform::from_xyz(-300.0, 0.0, 5.0),
        GlobalTransform::from_xyz(-300.0, 0.0, 5.0),
    ));

    commands.spawn((
        Name::new("Highlighted Target"),
        HighlightTarget,
        FovTarget::default(),
        FovStimulusSource::default(),
        Sprite::from_color(Color::srgb(0.56, 0.84, 0.94), Vec2::new(26.0, 32.0)),
        Transform::from_translation(TARGET_PATH[0]),
        GlobalTransform::from_translation(TARGET_PATH[0]),
    ));

    for (name, position) in [
        ("Side Target", Vec3::new(80.0, 160.0, 3.0)),
        ("Occluded Target", Vec3::new(360.0, 40.0, 3.0)),
    ] {
        commands.spawn((
            Name::new(name),
            FovTarget::default(),
            Sprite::from_color(Color::srgb(0.58, 0.62, 0.68), Vec2::splat(24.0)),
            Transform::from_translation(position),
            GlobalTransform::from_translation(position),
        ));
    }

    commands.spawn((
        Name::new("Signal Frame"),
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
            Name::new("Signal Fill"),
            SignalBarFill,
            Node {
                width: percent(0.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.24, 0.68, 0.92)),
        )],
    ));

    commands.spawn((
        Name::new("Example Label"),
        Text::new(
            "perception_kernel: top-right pane tunes the neutral signal model; the bar and target color show raw stimulus without any stealth-specific state machine.\nControls: use the pane to tune cone, focus, direct visibility scale, and indirect signal while the target loops through the arena.",
        ),
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

fn sync_pane(pane: Res<KernelPane>, mut sensor: Single<&mut SpatialFov, With<Sensor>>) {
    if !pane.is_changed() {
        return;
    }

    sensor.shape = saddle_ai_fov::SpatialShape::Cone {
        range: pane.range,
        half_angle_radians: pane.half_angle,
    };
    sensor.near_override = pane.near_override;
    sensor.stimulus.focused_half_angle_radians = pane.focus_half_angle;
}

fn animate_sensor(
    time: Res<Time>,
    pane: Res<KernelPane>,
    mut sensor: Single<(&mut Transform, &mut GlobalTransform), With<Sensor>>,
) {
    let angle = (time.elapsed_secs() * pane.sweep_speed).sin() * 0.9;
    sensor.0.rotation = Quat::from_rotation_z(angle);
    *sensor.1 = GlobalTransform::from(*sensor.0.as_ref());
}

fn animate_target(
    time: Res<Time>,
    pane: Res<KernelPane>,
    mut target: Single<(&mut Transform, &mut GlobalTransform), With<HighlightTarget>>,
) {
    let progress = time.elapsed_secs() * (0.18 + pane.sweep_speed * 0.18);
    let from = progress.floor() as usize % TARGET_PATH.len();
    let to = (from + 1) % TARGET_PATH.len();
    let t = progress.fract();
    let position = TARGET_PATH[from].lerp(TARGET_PATH[to], t);
    target.0.translation = position;
    *target.1 = GlobalTransform::from_translation(position);
}

fn update_stimulus_source(
    pane: Res<KernelPane>,
    target: Single<&Transform, With<HighlightTarget>>,
    mut source: Single<&mut FovStimulusSource, With<HighlightTarget>>,
) {
    let in_spotlight = target
        .translation
        .truncate()
        .distance(Vec2::new(200.0, 10.0))
        < 92.0;
    let on_grate = target.translation.x > 210.0
        && target.translation.x < 360.0
        && target.translation.y > 28.0
        && target.translation.y < 84.0;

    source.direct_visibility_scale = if in_spotlight {
        pane.spotlight_scale
    } else {
        pane.shadow_scale
    };
    source.indirect_signal = if on_grate { pane.indirect_signal } else { 0.0 };
}

fn update_presentation(
    mut pane: ResMut<KernelPane>,
    sensor: Single<&SpatialFovState, With<Sensor>>,
    target: Single<(Entity, &mut Sprite), With<HighlightTarget>>,
    bar: Single<(&mut Node, &mut BackgroundColor), With<SignalBarFill>>,
) {
    let (target_entity, mut sprite) = target.into_inner();
    let signal = sensor
        .stimulus_of(target_entity)
        .map(|entry| entry.signal)
        .unwrap_or(0.0);

    pane.highlighted_signal = signal;
    pane.visible_targets = sensor.visible_now.len();
    pane.remembered_targets = sensor.remembered.len();

    sprite.color = if signal >= 0.8 {
        Color::srgb(0.95, 0.24, 0.18)
    } else if signal >= 0.25 {
        Color::srgb(0.98, 0.86, 0.36)
    } else {
        Color::srgb(0.56, 0.84, 0.94)
    };

    let (mut fill, mut fill_color) = bar.into_inner();
    fill.width = percent((signal * 100.0).clamp(0.0, 100.0));
    fill_color.0 = if signal >= 0.8 {
        Color::srgb(0.90, 0.20, 0.16)
    } else if signal >= 0.25 {
        Color::srgb(0.94, 0.75, 0.28)
    } else {
        Color::srgb(0.24, 0.68, 0.92)
    };
}
