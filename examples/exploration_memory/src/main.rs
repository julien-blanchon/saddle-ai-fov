use saddle_ai_fov_example_support as support;

use bevy::prelude::*;
use saddle_ai_fov::{FovPlugin, GridFov, GridFovState, GridOpacityMap};
use saddle_pane::prelude::*;
use support::{
    GridCellSprite, apply_grid_visibility_colors, demo_grid_map, sample_path, spawn_grid_tiles,
};

#[derive(Component)]
struct Explorer;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "Exploration Memory", position = "top-right")]
struct ExplorationPane {
    #[pane]
    pause_motion: bool,
    #[pane(slider, min = 2.0, max = 8.0, step = 1.0)]
    viewer_radius: i32,
    #[pane(slider, min = 0.1, max = 1.0, step = 0.02)]
    viewer_speed: f32,
    #[pane(monitor)]
    visible_cells: usize,
    #[pane(monitor)]
    explored_cells: usize,
}

impl Default for ExplorationPane {
    fn default() -> Self {
        Self {
            pause_motion: false,
            viewer_radius: 4,
            viewer_speed: 0.42,
            visible_cells: 0,
            explored_cells: 0,
        }
    }
}

const EXPLORER_PATH: &[IVec2] = &[
    IVec2::new(2, 8),
    IVec2::new(2, 2),
    IVec2::new(7, 2),
    IVec2::new(7, 6),
    IVec2::new(11, 6),
    IVec2::new(11, 2),
    IVec2::new(13, 8),
    IVec2::new(2, 8),
];

fn main() {
    let grid = demo_grid_map(40.0);

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.06)))
        .insert_resource(grid)
        .insert_resource(ExplorationPane::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov exploration_memory".into(),
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
        .register_pane::<ExplorationPane>()
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            sync_controls.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            animate_viewer.before(saddle_ai_fov::FovSystems::MarkDirty),
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

    let start = support::grid_world_position(&grid.spec, EXPLORER_PATH[0], 4.0);
    commands.spawn((
        Name::new("Explorer"),
        Explorer,
        GridFov::new(4),
        Sprite {
            color: Color::srgb(0.92, 0.80, 0.28),
            custom_size: Some(Vec2::splat(grid.spec.cell_size.x * 0.58)),
            ..default()
        },
        Transform::from_translation(start),
        GlobalTransform::from_translation(start),
    ));

    commands.spawn((
        Name::new("Example Label"),
        Text::new(
            "exploration_memory: visible_now stays bright, explored cells remain readable.\nControls: use the top-right pane to pause motion and tune radius/speed.",
        ),
        Node {
            position_type: PositionType::Absolute,
            left: px(18.0),
            top: px(16.0),
            ..default()
        },
    ));
}

fn animate_viewer(
    time: Res<Time>,
    pane: Res<ExplorationPane>,
    grid: Res<GridOpacityMap>,
    mut viewer: Single<(&mut Transform, &mut GlobalTransform), With<Explorer>>,
) {
    if pane.pause_motion {
        return;
    }

    let position = sample_path(
        &grid.spec,
        EXPLORER_PATH,
        time.elapsed_secs(),
        pane.viewer_speed,
        4.0,
    );
    viewer.0.translation = position;
    *viewer.1 = GlobalTransform::from_translation(position);
}

fn sync_controls(pane: Res<ExplorationPane>, mut viewer: Single<&mut GridFov, With<Explorer>>) {
    if !pane.is_changed() {
        return;
    }

    viewer.config.radius = pane.viewer_radius.max(0);
}

fn tint_tiles(
    grid: Res<GridOpacityMap>,
    viewer: Single<&GridFovState, With<Explorer>>,
    mut tiles: Query<(&GridCellSprite, &mut Sprite)>,
) {
    apply_grid_visibility_colors(&grid, &viewer.visible_now, &viewer.explored, &mut tiles);
}

fn update_pane(viewer: Single<&GridFovState, With<Explorer>>, mut pane: ResMut<ExplorationPane>) {
    pane.visible_cells = viewer.visible_now.len();
    pane.explored_cells = viewer.explored.len();
}
