use bevy::prelude::*;

use super::components::PinballSceneEntity;
use super::layout::spawn_pinball_layout;
use super::profile::PinballProfile;
use super::systems::*;
use avian2d::prelude::*;
use bevy::ecs::system::RunSystemOnce;

#[derive(Resource)]
pub struct PinballSimulation(pub bool);

impl Default for PinballSimulation {
    fn default() -> Self {
        Self(true)
    }
}

fn simulation_running(simulation: Res<PinballSimulation>) -> bool {
    simulation.0
}

/// 编辑器预览复用游戏布局、碰撞与弹珠系统。
pub fn restart_pinball(world: &mut World) {
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<PinballSceneEntity>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.despawn(entity);
    }
    world.resource_mut::<Messages<CollisionStart>>().clear();
    world
        .resource_mut::<Messages<crate::events::ActionEvent>>()
        .clear();
    world
        .run_system_once(spawn_pinball_layout)
        .expect("重建弹珠机布局失败");
    world
        .run_system_once(spawn_initial_marbles)
        .expect("重建弹珠失败");
}

pub struct PinballPlugin;

impl Plugin for PinballPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PinballProfile>()
            .init_resource::<PinballSimulation>()
            .init_resource::<CircleMeshCache>()
            .add_systems(Startup, spawn_pinball_layout)
            .add_systems(Startup, spawn_initial_marbles.after(spawn_pinball_layout))
            .add_systems(
                Update,
                (
                    check_multiplier_collision,
                    check_action_zone_collision,
                    check_boost_collision,
                    contain_marbles,
                    assist_stuck_marbles,
                    update_marble_display,
                    sync_marble_text_position,
                )
                    .run_if(simulation_running),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::super::components::{Marble, PinballSpawnPoint};
    use super::*;
    use crate::events::ActionEvent;
    use crate::profiler::Profiler;

    #[test]
    fn preview_rebuilds_shared_layout_without_accumulating_entities() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::scene::ScenePlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<ColorMaterial>()
        .init_asset::<Font>()
        .init_resource::<Profiler>()
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(PinballSimulation(false))
        .add_message::<ActionEvent>()
        .add_plugins(PinballPlugin);
        app.update();
        for _ in 0..3 {
            restart_pinball(app.world_mut());
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<PinballSceneEntity>>()
                    .iter(app.world())
                    .count(),
                63
            );
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<Marble>>()
                    .iter(app.world())
                    .count(),
                4
            );
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<PinballSpawnPoint>>()
                    .iter(app.world())
                    .count(),
                4
            );
        }
    }
}
