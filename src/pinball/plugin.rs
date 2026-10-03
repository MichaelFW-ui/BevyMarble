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
                    cleanup_eliminated_marbles,
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
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::super::components::{Marble, MarbleText, PinballSpawnPoint};
    use super::*;
    use crate::colors::TeamColor;
    use crate::events::ActionEvent;
    use crate::profiler::Profiler;
    use crate::territory::GameOver;

    fn pinball_app() -> App {
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
        app
    }

    #[test]
    fn eliminated_team_marbles_and_text_are_cleared_and_skipped_when_rebuilding() {
        let mut app = pinball_app();
        app.init_resource::<GameOver>();
        app.update();
        let world = app.world_mut();
        let eliminated_marble = world
            .query::<(Entity, &Marble)>()
            .iter(world)
            .find(|(_, marble)| marble.team == TeamColor::Red)
            .unwrap()
            .0;
        let eliminated_text = world
            .query::<(Entity, &MarbleText)>()
            .iter(world)
            .find(|(_, text)| text.marble_entity == eliminated_marble)
            .unwrap()
            .0;
        world.resource_mut::<GameOver>().eliminated[TeamColor::Red.index()] = true;
        app.update();
        assert!(app.world().get_entity(eliminated_marble).is_err());
        assert!(app.world().get_entity(eliminated_text).is_err());
        for rebuild in [false, true] {
            if rebuild {
                restart_pinball(app.world_mut());
            }
            let world = app.world_mut();
            let teams: Vec<_> = world
                .query::<&Marble>()
                .iter(world)
                .map(|marble| marble.team)
                .collect();
            assert_eq!(teams.len(), 3);
            for team in [TeamColor::Blue, TeamColor::Green, TeamColor::Yellow] {
                assert!(teams.contains(&team));
            }
            assert_eq!(world.query::<&MarbleText>().iter(world).count(), 3);
        }
    }

    #[test]
    fn preview_rebuilds_shared_layout_without_accumulating_entities() {
        let mut app = pinball_app();
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
