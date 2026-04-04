use saddle_ai_fov_example_support as support;

use bevy::prelude::*;
use saddle_ai_fov::{
    FovPlugin, GridFov, GridFovState, GridOpacityMap, merge_grid_visibility,
};
use saddle_pane::prelude::*;
use support::{
    GridCellSprite, apply_grid_visibility_colors, demo_grid_map, sample_path, spawn_grid_tiles,
};

#[derive(Component)]
struct ScoutAlpha;

#[derive(Component)]
struct ScoutBeta;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "Multi Viewer FOV", position = "top-right")]
struct MultiViewerPane {
    #[pane]
    pause_motion: bool,
    #[pane(slider, min = 2.0, max = 8.0, step = 1.0)]
    alpha_radius: i32,
    #[pane(slider, min = 2.0, max = 8.0, step = 1.0)]
    beta_radius: i32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.02)]
    alpha_speed: f32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.02)]
    beta_speed: f32,
    #[pane(monitor)]
    alpha_visible: usize,
    #[pane(monitor)]
    beta_visible: usize,
    #[pane(monitor)]
    merged_visible: usize,
}

impl Default for MultiViewerPane {
    fn default() -> Self {
        Self {
            pause_motion: false,
            alpha_radius: 4,
            beta_radius: 4,
            alpha_speed: 0.48,
            beta_speed: 0.44,
            alpha_visible: 0,
            beta_visible: 0,
            merged_visible: 0,
        }
    }
}

const PATH_ALPHA: &[IVec2] = &[
    IVec2::new(2, 2),
    IVec2::new(6, 2),
    IVec2::new(6, 6),
    IVec2::new(2, 8),
];

const PATH_BETA: &[IVec2] = &[
    IVec2::new(12, 8),
    IVec2::new(12, 2),
    IVec2::new(8, 2),
    IVec2::new(8, 8),
];

fn main() {
    let grid = demo_grid_map(40.0);

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.045, 0.05)))
        .insert_resource(grid)
        .insert_resource(MultiViewerPane::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov multi_viewers".into(),
                resolution: (1180, 840).into(),
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
        .register_pane::<MultiViewerPane>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_controls.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_viewers.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            tint_tiles.after(saddle_ai_fov::FovSystems::Recompute),
        )
        .add_systems(
            Update,
            update_pane.after(saddle_ai_fov::FovSystems::Recompute),
        )
        .run();
}

fn setup(mut commands: Commands, grid: Res<GridOpacityMap>) {
    commands.spawn((Name::new("Example Camera"), Camera2d));
    spawn_grid_tiles(&mut commands, &grid);

    let alpha = support::grid_world_position(&grid.spec, PATH_ALPHA[0], 4.0);
    commands.spawn((
        Name::new("Scout Alpha"),
        ScoutAlpha,
        GridFov::new(4),
        Sprite {
            color: Color::srgb(0.30, 0.84, 0.95),
            custom_size: Some(Vec2::splat(grid.spec.cell_size.x * 0.58)),
            ..default()
        },
        Transform::from_translation(alpha),
        GlobalTransform::from_translation(alpha),
    ));

    let beta = support::grid_world_position(&grid.spec, PATH_BETA[0], 4.0);
    commands.spawn((
        Name::new("Scout Beta"),
        ScoutBeta,
        GridFov::new(4),
        Sprite {
            color: Color::srgb(0.99, 0.56, 0.28),
            custom_size: Some(Vec2::splat(grid.spec.cell_size.x * 0.58)),
            ..default()
        },
        Transform::from_translation(beta),
        GlobalTransform::from_translation(beta),
    ));

    commands.spawn((
        Name::new("Example Label"),
        Text::new("multi_viewers: merged party vision without coupling the viewers to each other"),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            ..default()
        },
    ));
}

fn animate_viewers(
    time: Res<Time>,
    pane: Res<MultiViewerPane>,
    grid: Res<GridOpacityMap>,
    mut alpha: Single<
        (&mut Transform, &mut GlobalTransform),
        (With<ScoutAlpha>, Without<ScoutBeta>),
    >,
    mut beta: Single<
        (&mut Transform, &mut GlobalTransform),
        (With<ScoutBeta>, Without<ScoutAlpha>),
    >,
) {
    if pane.pause_motion {
        return;
    }

    let alpha_pos = sample_path(
        &grid.spec,
        PATH_ALPHA,
        time.elapsed_secs(),
        pane.alpha_speed,
        4.0,
    );
    let beta_pos = sample_path(
        &grid.spec,
        PATH_BETA,
        time.elapsed_secs() + 2.0,
        pane.beta_speed,
        4.0,
    );

    alpha.0.translation = alpha_pos;
    *alpha.1 = GlobalTransform::from_translation(alpha_pos);
    beta.0.translation = beta_pos;
    *beta.1 = GlobalTransform::from_translation(beta_pos);
}

fn sync_controls(
    pane: Res<MultiViewerPane>,
    mut alpha: Single<&mut GridFov, (With<ScoutAlpha>, Without<ScoutBeta>)>,
    mut beta: Single<&mut GridFov, (With<ScoutBeta>, Without<ScoutAlpha>)>,
) {
    if !pane.is_changed() {
        return;
    }

    alpha.config.radius = pane.alpha_radius.max(0);
    beta.config.radius = pane.beta_radius.max(0);
}

fn tint_tiles(
    grid: Res<GridOpacityMap>,
    viewers: Query<&GridFovState>,
    mut tiles: Query<(&GridCellSprite, &mut Sprite)>,
) {
    let merged = merge_grid_visibility(viewers.iter());
    apply_grid_visibility_colors(&grid, &merged, &merged, &mut tiles);
}

fn update_pane(
    alpha: Single<&GridFovState, With<ScoutAlpha>>,
    beta: Single<&GridFovState, With<ScoutBeta>>,
    mut pane: ResMut<MultiViewerPane>,
) {
    let merged_visible = alpha
        .visible_now
        .iter()
        .chain(beta.visible_now.iter())
        .copied()
        .collect::<std::collections::HashSet<_>>();

    pane.alpha_visible = alpha.visible_now.len();
    pane.beta_visible = beta.visible_now.len();
    pane.merged_visible = merged_visible.len();
}
