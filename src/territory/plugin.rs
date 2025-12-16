use bevy::prelude::*;

use super::grid::TerritoryGrid;
use super::render::*;
use super::setup::*;
use super::systems::*;

#[derive(Resource, Debug, Clone, Copy)]
pub struct TerritorySettings {
    pub enable_bullet_bullet_collision: bool,
}

impl Default for TerritorySettings {
    fn default() -> Self {
        Self {
            enable_bullet_bullet_collision: false,
        }
    }
}

pub struct TerritoryPlugin;

impl Plugin for TerritoryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerritoryGrid::new(1024, 1024))
            .insert_resource(TerritorySettings::default())
            .init_resource::<BulletPaintKernel>()
            .init_resource::<TargetSpatialIndex>()
            .add_systems(Startup, (setup_grid_render, setup_initial_game))
            .add_systems(
                Update,
                (
                    spawn_units_from_events,
                    machine_gun_rotate_fire,
                    update_target_spatial_index,
                    ciws_target_fire,
                    bullet_move,
                    bullet_hit_terrain,
                    bullet_hit_units,
                    bullet_bullet_collision,
                    bigball_occupy_territory,
                    bigball_collision,
                    cleanup_depleted_units,
                    check_victory,
                    contain_units,
                    update_grid_render,
                    update_bigball_value_text,
                ),
            )
            .add_systems(PostUpdate, sync_bullet_rotation_to_velocity);
    }
}
