use std::collections::HashMap;

use bevy::{
    ecs::{intern::Interned, schedule::ScheduleLabel},
    prelude::*,
};

use crate::{
    FovSystems,
    components::SpatialFovState,
    messages::{StealthAwarenessChanged, StealthTargetDetected, StealthTargetLost},
};

#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum StealthAwarenessSystems {
    Prepare,
    Sync,
}

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct StealthAwarenessConfig {
    pub alert_threshold: f32,
}

impl Default for StealthAwarenessConfig {
    fn default() -> Self {
        Self {
            alert_threshold: 0.8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum StealthAwarenessLevel {
    Unaware,
    Suspicious,
    Alert,
    Searching,
    Lost,
}

#[derive(Component, Debug, Clone, Default, PartialEq, Reflect)]
#[reflect(Component)]
pub struct StealthAwarenessState {
    pub entries: Vec<StealthAwarenessEntry>,
}

impl StealthAwarenessState {
    pub fn awareness_of(&self, entity: Entity) -> Option<&StealthAwarenessEntry> {
        self.entries.iter().find(|entry| entry.entity == entity)
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
pub struct StealthAwarenessEntry {
    pub entity: Entity,
    pub level: StealthAwarenessLevel,
    pub signal: f32,
    pub visibility_score: f32,
    pub currently_visible: bool,
    pub focused: bool,
    pub last_seen_seconds_ago: Option<f32>,
    pub last_known_position: Option<Vec3>,
}

pub struct StealthAwarenessPlugin {
    pub update_schedule: Interned<dyn ScheduleLabel>,
}

impl StealthAwarenessPlugin {
    pub fn new(update_schedule: impl ScheduleLabel) -> Self {
        Self {
            update_schedule: update_schedule.intern(),
        }
    }
}

impl Default for StealthAwarenessPlugin {
    fn default() -> Self {
        Self::new(Update)
    }
}

impl Plugin for StealthAwarenessPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StealthAwarenessChanged>()
            .add_message::<StealthTargetDetected>()
            .add_message::<StealthTargetLost>()
            .register_type::<StealthAwarenessConfig>()
            .register_type::<StealthAwarenessEntry>()
            .register_type::<StealthAwarenessLevel>()
            .register_type::<StealthAwarenessState>()
            .configure_sets(
                self.update_schedule,
                (
                    StealthAwarenessSystems::Prepare,
                    StealthAwarenessSystems::Sync,
                )
                    .chain(),
            )
            .add_systems(
                self.update_schedule,
                prepare_stealth_state
                    .in_set(StealthAwarenessSystems::Prepare)
                    .after(FovSystems::Prepare),
            )
            .add_systems(
                self.update_schedule,
                sync_stealth_state
                    .in_set(StealthAwarenessSystems::Sync)
                    .after(FovSystems::Recompute),
            );
    }
}

fn prepare_stealth_state(
    mut commands: Commands,
    viewers: Query<(Entity, Option<&StealthAwarenessState>), With<StealthAwarenessConfig>>,
) {
    for (entity, state) in &viewers {
        if state.is_none() {
            commands
                .entity(entity)
                .insert(StealthAwarenessState::default());
        }
    }
}

fn sync_stealth_state(
    mut viewers: Query<(
        Entity,
        &SpatialFovState,
        &StealthAwarenessConfig,
        &mut StealthAwarenessState,
    )>,
    mut awareness_changed: MessageWriter<StealthAwarenessChanged>,
    mut target_detected: MessageWriter<StealthTargetDetected>,
    mut target_lost: MessageWriter<StealthTargetLost>,
) {
    for (viewer, spatial_state, config, mut stealth_state) in &mut viewers {
        let previous: HashMap<_, _> = stealth_state
            .entries
            .iter()
            .cloned()
            .map(|entry| (entry.entity, entry))
            .collect();

        let mut next = spatial_state
            .stimuli
            .iter()
            .map(|entry| StealthAwarenessEntry {
                entity: entry.entity,
                level: classify_stealth_level(entry, config.alert_threshold),
                signal: entry.signal,
                visibility_score: entry.visibility_score,
                currently_visible: entry.currently_visible,
                focused: entry.focused,
                last_seen_seconds_ago: entry.last_seen_seconds_ago,
                last_known_position: entry.last_known_position,
            })
            .collect::<Vec<_>>();
        next.sort_by_key(|entry| entry.entity.index());

        emit_stealth_messages(
            viewer,
            &previous,
            &next,
            &mut awareness_changed,
            &mut target_detected,
            &mut target_lost,
        );
        stealth_state.entries = next;
    }
}

fn classify_stealth_level(
    entry: &crate::stimulus::SpatialStimulusEntry,
    alert_threshold: f32,
) -> StealthAwarenessLevel {
    if entry.currently_visible {
        if entry.signal >= alert_threshold.max(0.0) {
            StealthAwarenessLevel::Alert
        } else {
            StealthAwarenessLevel::Suspicious
        }
    } else if entry.last_seen_seconds_ago.is_some() {
        if entry.signal > 0.0 {
            StealthAwarenessLevel::Searching
        } else {
            StealthAwarenessLevel::Lost
        }
    } else if entry.signal > 0.0 {
        StealthAwarenessLevel::Suspicious
    } else {
        StealthAwarenessLevel::Unaware
    }
}

fn emit_stealth_messages(
    viewer: Entity,
    previous: &HashMap<Entity, StealthAwarenessEntry>,
    next: &[StealthAwarenessEntry],
    awareness_changed: &mut MessageWriter<StealthAwarenessChanged>,
    target_detected: &mut MessageWriter<StealthTargetDetected>,
    target_lost: &mut MessageWriter<StealthTargetLost>,
) {
    let next_map: HashMap<_, _> = next
        .iter()
        .cloned()
        .map(|entry| (entry.entity, entry))
        .collect();

    for (target, entry) in &next_map {
        let previous_level = previous
            .get(target)
            .map(|previous| previous.level)
            .unwrap_or(StealthAwarenessLevel::Unaware);
        if previous_level != entry.level {
            awareness_changed.write(StealthAwarenessChanged {
                viewer,
                target: *target,
                previous_level,
                level: entry.level,
                signal: entry.signal,
                last_known_position: entry.last_known_position,
            });
        }

        if previous_level != StealthAwarenessLevel::Alert
            && entry.level == StealthAwarenessLevel::Alert
        {
            target_detected.write(StealthTargetDetected {
                viewer,
                target: *target,
                signal: entry.signal,
                last_known_position: entry.last_known_position,
            });
        }
    }

    for (target, entry) in previous {
        let next_level = next_map
            .get(target)
            .map(|next| next.level)
            .unwrap_or(StealthAwarenessLevel::Unaware);
        if entry.level == StealthAwarenessLevel::Alert && next_level != StealthAwarenessLevel::Alert
        {
            let next_entry = next_map.get(target);
            target_lost.write(StealthTargetLost {
                viewer,
                target: *target,
                signal: next_entry.map(|next| next.signal).unwrap_or(0.0),
                last_known_position: next_entry
                    .and_then(|next| next.last_known_position)
                    .or(entry.last_known_position),
            });
        }
    }
}

#[cfg(test)]
#[path = "stealth_tests.rs"]
mod tests;
