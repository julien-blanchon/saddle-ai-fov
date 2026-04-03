mod awareness;
mod algorithms;
mod components;
mod config;
mod debug;
mod grid;
mod messages;
mod resources;
mod spatial;
mod systems;

pub use awareness::{AwarenessLevel, SpatialAwarenessConfig, SpatialAwarenessEntry};
pub use crate::algorithms::los::{has_grid_line_of_sight, supercover_line};
pub use crate::algorithms::shadowcasting::compute_grid_fov;
pub use components::{
    FovDirty, FovOccluder, FovPerceptionModifiers, FovTarget, GridFov, GridFovState, SpatialFov,
    SpatialFovState,
};
pub use config::FovRuntimeConfig;
pub use debug::{FovDebugGizmos, FovDebugSettings};
pub use grid::{
    GridCornerPolicy, GridFovBackend, GridFovConfig, GridFovResult, GridMapSpec, GridOpacityMap,
    merge_grid_visibility,
};
pub use resources::FovStats;
pub use spatial::{
    OccluderShape, SpatialDimension, SpatialShape, SpatialVisibilityQuery, VisibilityLayer,
    VisibilityLayerMask, VisibilityTestResult, WorldOccluder, evaluate_visibility,
    merge_spatial_visibility, occluded_by_any,
};
pub use messages::{SpatialAwarenessChanged, SpatialTargetDetected, SpatialTargetLost};

use bevy::{
    app::PostStartup,
    ecs::{intern::Interned, schedule::ScheduleLabel},
    prelude::*,
};

#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum FovSystems {
    Prepare,
    MarkDirty,
    Recompute,
    DebugDraw,
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct NeverDeactivateSchedule;

pub struct FovPlugin {
    pub activate_schedule: Interned<dyn ScheduleLabel>,
    pub deactivate_schedule: Interned<dyn ScheduleLabel>,
    pub update_schedule: Interned<dyn ScheduleLabel>,
    pub config: FovRuntimeConfig,
}

impl FovPlugin {
    pub fn new(
        activate_schedule: impl ScheduleLabel,
        deactivate_schedule: impl ScheduleLabel,
        update_schedule: impl ScheduleLabel,
    ) -> Self {
        Self {
            activate_schedule: activate_schedule.intern(),
            deactivate_schedule: deactivate_schedule.intern(),
            update_schedule: update_schedule.intern(),
            config: FovRuntimeConfig::default(),
        }
    }

    pub fn always_on(update_schedule: impl ScheduleLabel) -> Self {
        Self::new(PostStartup, NeverDeactivateSchedule, update_schedule)
    }

    pub fn with_config(mut self, config: FovRuntimeConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for FovPlugin {
    fn default() -> Self {
        Self::always_on(Update)
    }
}

impl Plugin for FovPlugin {
    fn build(&self, app: &mut App) {
        if self.deactivate_schedule == NeverDeactivateSchedule.intern() {
            app.init_schedule(NeverDeactivateSchedule);
        }

        if !app.world().contains_resource::<FovRuntimeConfig>() {
            app.insert_resource(self.config.clone());
        }

        app.init_resource::<FovStats>()
            .init_resource::<FovDebugSettings>()
            .init_resource::<systems::FovRuntimeState>()
            .add_message::<SpatialAwarenessChanged>()
            .add_message::<SpatialTargetDetected>()
            .add_message::<SpatialTargetLost>()
            .register_type::<AwarenessLevel>()
            .register_type::<FovDebugSettings>()
            .register_type::<FovDirty>()
            .register_type::<FovOccluder>()
            .register_type::<FovPerceptionModifiers>()
            .register_type::<FovTarget>()
            .register_type::<FovRuntimeConfig>()
            .register_type::<FovStats>()
            .register_type::<GridCornerPolicy>()
            .register_type::<GridFov>()
            .register_type::<GridFovBackend>()
            .register_type::<GridFovConfig>()
            .register_type::<GridFovState>()
            .register_type::<GridMapSpec>()
            .register_type::<GridOpacityMap>()
            .register_type::<OccluderShape>()
            .register_type::<SpatialAwarenessConfig>()
            .register_type::<SpatialAwarenessEntry>()
            .register_type::<SpatialDimension>()
            .register_type::<SpatialFov>()
            .register_type::<SpatialFovState>()
            .register_type::<SpatialShape>()
            .register_type::<SpatialVisibilityQuery>()
            .register_type::<VisibilityLayer>()
            .register_type::<VisibilityLayerMask>()
            .configure_sets(
                self.update_schedule,
                (
                    FovSystems::Prepare,
                    FovSystems::MarkDirty,
                    FovSystems::Recompute,
                    FovSystems::DebugDraw,
                )
                    .chain(),
            )
            .add_systems(self.activate_schedule, systems::activate_runtime)
            .add_systems(self.deactivate_schedule, systems::deactivate_runtime)
            .add_systems(
                self.update_schedule,
                systems::prepare_runtime
                    .in_set(FovSystems::Prepare)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::mark_viewers_dirty
                    .in_set(FovSystems::MarkDirty)
                    .run_if(systems::runtime_is_active),
            )
            .add_systems(
                self.update_schedule,
                systems::recompute_viewers
                    .in_set(FovSystems::Recompute)
                    .run_if(systems::runtime_is_active),
            );

        app.add_systems(
            self.update_schedule,
            systems::draw_debug
                .in_set(FovSystems::DebugDraw)
                .run_if(systems::runtime_is_active)
                .run_if(systems::debug_enabled),
        );
    }
}
