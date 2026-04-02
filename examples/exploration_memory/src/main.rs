use saddle_ai_fov_example_support as support;

use bevy::prelude::*;
use saddle_ai_saddle_ai_fov::{FovPlugin, GridFov, GridFovState, GridOpacityMap};
use support::{
    GridCellSprite, apply_grid_visibility_colors, demo_grid_map, sample_path, spawn_grid_tiles,
};

#[derive(Component)]
struct Explorer;

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
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov exploration_memory".into(),
                resolution: (1180, 840).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FovPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            animate_viewer.before(saddle_ai_fov::FovSystems::MarkDirty),
        )
        .add_systems(
            Update,
            tint_tiles.after(saddle_ai_fov::FovSystems::Recompute),
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
        Text::new("exploration_memory: visible_now stays bright, explored cells remain readable"),
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
    grid: Res<GridOpacityMap>,
    mut viewer: Single<(&mut Transform, &mut GlobalTransform), With<Explorer>>,
) {
    let position = sample_path(&grid.spec, EXPLORER_PATH, time.elapsed_secs(), 0.42, 4.0);
    viewer.0.translation = position;
    *viewer.1 = GlobalTransform::from_translation(position);
}

fn tint_tiles(
    grid: Res<GridOpacityMap>,
    viewer: Single<&GridFovState, With<Explorer>>,
    mut tiles: Query<(&GridCellSprite, &mut Sprite)>,
) {
    apply_grid_visibility_colors(&grid, &viewer.visible_now, &viewer.explored, &mut tiles);
}
