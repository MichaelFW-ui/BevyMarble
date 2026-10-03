use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResourcePlugin;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::sprite_render::Material2dPlugin;
use bevy::time::Fixed;

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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetApp;
    use bevy::time::TimeUpdateStrategy;
    use bevy_transform_interpolation::prelude::TransformInterpolationPlugin;
    use crate::colors::TeamColor;
    use crate::events::{ActionEvent, UnitDestroyedEvent, VictoryEvent};
    use crate::profiler::Profiler;
    use super::super::components::*;
    use std::time::Duration;

    #[test]
    fn bullet_keeps_moving_between_fixed_steps_in_the_game_schedule() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformInterpolationPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<ColorMaterial>()
            .init_asset::<Font>()
            .init_resource::<Profiler>()
            .add_message::<ActionEvent>()
            .add_message::<VictoryEvent>()
            .add_message::<UnitDestroyedEvent>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 240.0)))
            .add_plugins(TerritoryPlugin);
        app.finish();
        app.cleanup();
        let bullet = app.world_mut().spawn((
            Bullet { team: TeamColor::Red, value: 100_000 },
            LogicPosition(Vec2::ZERO), LastLogicPosition(Vec2::ZERO), BulletPrevPosition(Vec2::ZERO),
            KinematicVelocity(Vec2::new(250.0, 0.0)),
            bevy_transform_interpolation::prelude::TranslationInterpolation,
            Transform::default(),
        )).id();
        for _ in 0..8 { app.update(); }
        let mut previous_x = app.world().get::<Transform>(bullet).unwrap().translation.x;
        for _ in 0..16 {
            app.update();
            let x = app.world().get::<Transform>(bullet).unwrap().translation.x;
            assert!((x - previous_x - 250.0 / 240.0).abs() < 0.001,
                "子弹在固定步之间停顿或跳跃：{previous_x} -> {x}");
            previous_x = x;
        }
    }
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
    pub eliminated: [bool; 4],
    pub finished: bool,
}

pub struct TerritoryPlugin;

impl Plugin for TerritoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<GridMaterial>::default())
            .add_plugins(ExtractResourcePlugin::<GridUpload>::default())
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(TerritoryGrid::new(1024, 1024))
            .insert_resource(TerritorySettings::default())
            .init_resource::<GameOver>()
            .init_resource::<BulletPaintKernel>()
            .init_resource::<TargetSpatialIndex>()
            .init_resource::<CollisionSpatialIndex>()
            .init_resource::<PendingDespawns>()
            .init_resource::<GridFrameBuffer>()
            .add_systems(Startup, (setup_territory_assets, setup_grid_render, setup_initial_game))
            .add_systems(
                FixedUpdate,
                (
                    clear_pending_despawns,
                    machine_gun_rotate_fire,
                    bigball_integrate,
                    bullet_integrate,
                    sync_bullet_rotation_to_velocity,
                    update_collision_spatial_index,
                    update_target_spatial_index.run_if(|settings: Res<TerritorySettings>| settings.enable_ciws),
                    ciws_target_fire.run_if(|settings: Res<TerritorySettings>| settings.enable_ciws),
                    bullet_hit_units_manual,
                    bullet_hit_terrain,
                    bullet_bullet_collision_manual,
                    bigball_collision,
                    bigball_hit_shield,
                    bigball_hit_hq,
                    eliminate_defeated_teams,
                    bigball_occupy_territory,
                    cleanup_depleted_units,
                    check_victory,
                    capture_grid_frame,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    spawn_units_from_events,
                    update_grid_render,
                    update_bigball_value_text,
                    show_game_state,
                    handle_unit_destroyed_event,
                ),
            );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Render, upload_grid_texture.in_set(RenderSystems::Prepare));
        }
    }
}
