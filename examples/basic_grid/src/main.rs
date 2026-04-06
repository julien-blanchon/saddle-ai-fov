use saddle_ai_fov_example_support as support;

use bevy::prelude::*;
use saddle_ai_fov::{FovPlugin, GridFov, GridFovState, GridOpacityMap};
use saddle_pane::prelude::*;
use support::{
    GridCellSprite, apply_grid_visibility_colors, demo_grid_map, sample_path, spawn_grid_tiles,
};

#[derive(Component)]
struct PrimaryViewer;

#[derive(Resource, Debug, Clone, Copy, Pane)]
#[pane(title = "Basic Grid FOV", position = "top-right")]
struct BasicGridPane {
    #[pane]
    pause_motion: bool,
    #[pane(slider, min = 2.0, max = 8.0, step = 1.0)]
    viewer_radius: i32,
    #[pane(slider, min = 0.1, max = 1.2, step = 0.02)]
    viewer_speed: f32,
    #[pane(monitor)]
    visible_cells: usize,
    #[pane(monitor)]
    explored_cells: usize,
}

impl Default for BasicGridPane {
    fn default() -> Self {
        Self {
            pause_motion: false,
            viewer_radius: 5,
            viewer_speed: 0.55,
            visible_cells: 0,
            explored_cells: 0,
        }
    }
}

const VIEWER_PATH: &[IVec2] = &[
    IVec2::new(2, 2),
    IVec2::new(5, 2),
    IVec2::new(5, 5),
    IVec2::new(10, 5),
    IVec2::new(10, 8),
    IVec2::new(3, 8),
    IVec2::new(2, 2),
];

fn main() {
    let grid = demo_grid_map(44.0);

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
        .insert_resource(grid)
        .insert_resource(BasicGridPane::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "fov basic_grid".into(),
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
        .register_pane::<BasicGridPane>()
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

    commands.spawn((
        Name::new("Primary Viewer"),
        PrimaryViewer,
        GridFov::new(5),
        Sprite {
            color: Color::srgb(0.94, 0.43, 0.24),
            custom_size: Some(Vec2::splat(grid.spec.cell_size.x * 0.6)),
            ..default()
        },
        Transform::from_translation(support::grid_world_position(
            &grid.spec,
            VIEWER_PATH[0],
            4.0,
        )),
        GlobalTransform::from_translation(support::grid_world_position(
            &grid.spec,
            VIEWER_PATH[0],
            4.0,
        )),
    ));

    commands.spawn((
        Name::new("Example Label"),
        Text::new(
            "basic_grid: recursive shadowcasting on a reusable GridOpacityMap.\nControls: use the top-right pane to pause motion and tune radius/speed.",
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
    pane: Res<BasicGridPane>,
    grid: Res<GridOpacityMap>,
    mut viewer: Single<(&mut Transform, &mut GlobalTransform), With<PrimaryViewer>>,
) {
    if pane.pause_motion {
        return;
    }

    let position = sample_path(
        &grid.spec,
        VIEWER_PATH,
        time.elapsed_secs(),
        pane.viewer_speed,
        4.0,
    );
    viewer.0.translation = position;
    *viewer.1 = GlobalTransform::from_translation(position);
}

fn sync_controls(pane: Res<BasicGridPane>, mut viewer: Single<&mut GridFov, With<PrimaryViewer>>) {
    if !pane.is_changed() {
        return;
    }

    viewer.config.radius = pane.viewer_radius.max(0);
}

fn tint_tiles(
    grid: Res<GridOpacityMap>,
    viewer: Single<&GridFovState, With<PrimaryViewer>>,
    mut tiles: Query<(&GridCellSprite, &mut Sprite)>,
) {
    apply_grid_visibility_colors(&grid, &viewer.visible_now, &viewer.explored, &mut tiles);
}

fn update_pane(
    viewer: Single<&GridFovState, With<PrimaryViewer>>,
    mut pane: ResMut<BasicGridPane>,
) {
    pane.visible_cells = viewer.visible_now.len();
    pane.explored_cells = viewer.explored.len();
}
