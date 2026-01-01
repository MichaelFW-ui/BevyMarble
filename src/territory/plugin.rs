use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

use super::grid::TerritoryGrid;
use super::render::*;
use super::setup::*;
use super::systems::*;

#[derive(Resource, Debug, Clone, Copy)]
pub struct TerritorySettings {
    pub enable_bullet_bullet_collision: bool,
    pub enable_ciws: bool,
    /// CIWS 是否把敌方 `Bullet` 也当作目标（会显著增加索引与查询开销）
    pub ciws_target_bullets: bool,
    pub ciws_distance_metric: CiwsDistanceMetric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiwsDistanceMetric {
    /// L1 距离：`|dx| + |dy|`（更便宜，但方向有偏好）
    Manhattan,
    /// L2 距离的平方：`dx*dx + dy*dy`（更接近“真正最近”）
    EuclideanSquared,
}

impl Default for TerritorySettings {
    fn default() -> Self {
        Self {
            enable_bullet_bullet_collision: false,
            enable_ciws: true,
            ciws_target_bullets: false,
            ciws_distance_metric: CiwsDistanceMetric::Manhattan,
        }
    }
}

/// 游戏结束状态
#[derive(Resource, Debug, Clone, Default)]
pub struct GameOver {
    pub winner: Option<crate::colors::TeamColor>,
}

pub struct TerritoryPlugin;

impl Plugin for TerritoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GridMaterial>::default())
            .insert_resource(TerritoryGrid::new(1024, 1024))
            .insert_resource(TerritorySettings::default())
            .init_resource::<GameOver>()
            .init_resource::<BulletPaintKernel>()
            .init_resource::<BigBallPaintKernel>()
            .init_resource::<TargetSpatialIndex>()
            .add_systems(Startup, (setup_territory_assets, setup_grid_render, setup_initial_game))
            .add_systems(
                FixedUpdate,
                (
                    machine_gun_rotate_fire,
                    update_target_spatial_index.run_if(|settings: Res<TerritorySettings>| settings.enable_ciws),
                    ciws_target_fire.run_if(|settings: Res<TerritorySettings>| settings.enable_ciws),
                    bigball_integrate,
                    bullet_integrate,
                    bullet_hit_terrain,
                    bullet_hit_units_manual,
                    bullet_bullet_collision_manual,
                    bigball_occupy_territory,
                    bigball_collision,
                    bigball_hit_hq,
                    cleanup_depleted_units,
                    check_victory,
                ),
            )
            .add_systems(
                Update,
                (
                    spawn_units_from_events,
                    update_grid_render,
                    update_bigball_value_text,
                    handle_victory_event,
                    handle_unit_destroyed_event,
                ),
            )
            .add_systems(PostUpdate, sync_bullet_rotation_to_velocity);
    }
}
