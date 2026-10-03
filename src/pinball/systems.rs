use avian2d::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

use super::components::*;
use super::profile::{MarbleSettings, PinballProfile};
use super::utils::{calculate_radius, format_value};
use crate::colors::TeamColor;
use crate::events::{ActionEvent, ActionType};
use crate::profiler::{CounterId, Profiler, ScopeId};
use crate::territory::GameOver;

const STUCK_TIME_SECS: f32 = 1.5;
const STUCK_MOVE_EPS: f32 = 0.8;
const STUCK_SPEED_EPS: f32 = 5.0;

#[derive(Resource, Default)]
pub struct CircleMeshCache {
    by_radius_bits: HashMap<u32, Handle<Mesh>>,
}

impl CircleMeshCache {
    fn circle(&mut self, meshes: &mut Assets<Mesh>, radius: f32) -> Handle<Mesh> {
        let key = radius.to_bits();
        if let Some(handle) = self.by_radius_bits.get(&key) {
            return handle.clone();
        }
        let handle = meshes.add(Circle::new(radius));
        self.by_radius_bits.insert(key, handle.clone());
        handle
    }
}

/// 生成初始弹珠
pub fn spawn_initial_marbles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh_cache: ResMut<CircleMeshCache>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform)>,
    asset_server: Res<AssetServer>,
    profile: Res<PinballProfile>,
    game_over: Option<Res<GameOver>>,
) {
    for (spawn_point, transform) in spawn_points.iter() {
        if game_over
            .as_ref()
            .is_some_and(|state| state.eliminated[spawn_point.team.index()])
        {
            continue;
        }
        let material = materials.add(spawn_point.team.to_color());
        spawn_marble(
            &mut commands,
            &mut *meshes,
            &mut mesh_cache,
            material,
            spawn_point.team,
            transform.translation.truncate(),
            &asset_server,
            &profile.marble,
        );
    }
}

/// 队伍出局后清理弹珠及数值文字，在碰撞处理与重置前执行。
pub fn cleanup_eliminated_marbles(
    mut commands: Commands,
    game_over: Option<Res<GameOver>>,
    marbles: Query<(Entity, &Marble)>,
    texts: Query<(Entity, &MarbleText)>,
) {
    let Some(game_over) = game_over else {
        return;
    };
    for (entity, marble) in &marbles {
        if game_over.eliminated[marble.team.index()] {
            commands.entity(entity).despawn();
        }
    }
    for (entity, text) in &texts {
        if marbles.get(text.marble_entity).map_or(true, |(_, marble)| {
            game_over.eliminated[marble.team.index()]
        }) {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_marble(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mesh_cache: &mut CircleMeshCache,
    material: Handle<ColorMaterial>,
    team: TeamColor,
    position: Vec2,
    asset_server: &Res<AssetServer>,
    settings: &MarbleSettings,
) {
    // 给小球一个随机的初始水平速度
    let initial_velocity = spawn_velocity(settings);

    let marble = Marble {
        team,
        value: settings.initial_value,
    };
    let radius = calculate_radius(marble.value);
    let mesh = mesh_cache.circle(meshes, radius);

    let marble_entity = commands
        .spawn((
            PinballSceneEntity,
            marble.clone(),
            RenderLayers::layer(0),
            StuckMarbleTracker {
                last_pos: position,
                still_time: 0.0,
            },
            RigidBody::Dynamic,
            Collider::circle(radius),
            Restitution::new(settings.restitution),
            Friction::new(settings.friction),
            LinearVelocity(initial_velocity),
            TranslationInterpolation,
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_translation(position.extend(0.5)),
            CollisionEventsEnabled,
        ))
        .id();

    // 添加数值文本
    let text_value = format_value(marble.value);
    commands.spawn((
        PinballSceneEntity,
        MarbleText { marble_entity },
        RenderLayers::layer(0),
        Text2d::new(text_value),
        TextFont {
            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_translation(position.extend(1.0)),
    ));
}

/// 检测长期几乎不动的弹珠，给一个向上的升力避免卡死
pub fn assist_stuck_marbles(
    profiler: Res<Profiler>,
    time: Res<Time>,
    profile: Res<PinballProfile>,
    mut marbles: Query<(&Transform, &mut LinearVelocity, &mut StuckMarbleTracker), With<Marble>>,
) {
    let _scope = profiler.scope(ScopeId::PinballAssistStuckMarbles);
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let mut rng = rand::thread_rng();

    for (transform, mut velocity, mut tracker) in marbles.iter_mut() {
        let pos = transform.translation.truncate();
        let moved = pos.distance(tracker.last_pos);
        let speed = velocity.0.length();

        if moved < STUCK_MOVE_EPS && speed < STUCK_SPEED_EPS {
            tracker.still_time += dt;
        } else {
            tracker.still_time = 0.0;
            tracker.last_pos = pos;
        }

        if tracker.still_time >= STUCK_TIME_SECS {
            velocity.0.y = velocity.0.y.max(profile.marble.rescue_speed);
            velocity.0.x += rng.gen_range(-60.0..60.0);
            tracker.still_time = 0.0;
            tracker.last_pos = pos;
        }
    }
}

/// 检测弹珠进入加倍区域
pub fn check_multiplier_collision(
    profiler: Res<Profiler>,
    mut collision_started: MessageReader<CollisionStart>,
    mut marbles: Query<(&mut Marble, &mut Transform, &mut LinearVelocity)>,
    multiplier_zones: Query<&MultiplierZone>,
    profile: Res<PinballProfile>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
) {
    let _scope = profiler.scope(ScopeId::PinballCheckMultiplierCollision);
    let profiling = profiler.is_enabled();
    let mut read_events = 0u64;
    for event in collision_started.read() {
        if profiling {
            read_events += 1;
        }
        // 检查是否是弹珠和加倍区碰撞
        let (marble_entity, zone_entity) = if marbles.contains(event.collider1)
            && multiplier_zones.contains(event.collider2)
        {
            (event.collider1, event.collider2)
        } else if marbles.contains(event.collider2) && multiplier_zones.contains(event.collider1) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };

        if let (Ok((mut marble, mut transform, mut velocity)), Ok(zone)) = (
            marbles.get_mut(marble_entity),
            multiplier_zones.get(zone_entity),
        ) {
            // 加倍
            marble.multiply(zone.multiplier);

            if !zone.reset_position {
                continue;
            }

            // 找到对应颜色的起始点并重置位置
            for (spawn_point, spawn_transform) in spawn_points.iter() {
                if spawn_point.team == marble.team {
                    transform.translation = spawn_transform.translation;
                    // 重置速度
                    velocity.0 = spawn_velocity(&profile.marble);
                    break;
                }
            }
        }
    }
    profiler.add_counter(CounterId::PinballCollisionStartRead, read_events);
}

/// 检测弹珠进入行动选择区域
pub fn check_action_zone_collision(
    profiler: Res<Profiler>,
    mut collision_started: MessageReader<CollisionStart>,
    mut marbles: Query<(Entity, &mut Marble, &mut Transform, &mut LinearVelocity)>,
    action_zones: Query<&ActionZone>,
    profile: Res<PinballProfile>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
    mut action_events: MessageWriter<ActionEvent>,
) {
    let _scope = profiler.scope(ScopeId::PinballCheckActionZoneCollision);
    let profiling = profiler.is_enabled();
    let mut read_events = 0u64;
    for event in collision_started.read() {
        if profiling {
            read_events += 1;
        }
        // 检查是否是弹珠和行动区碰撞
        let (marble_entity, zone_entity) =
            if marbles.contains(event.collider1) && action_zones.contains(event.collider2) {
                (event.collider1, event.collider2)
            } else if marbles.contains(event.collider2) && action_zones.contains(event.collider1) {
                (event.collider2, event.collider1)
            } else {
                continue;
            };

        if let (Ok((_entity, mut marble, mut transform, mut velocity)), Ok(zone)) = (
            marbles.get_mut(marble_entity),
            action_zones.get(zone_entity),
        ) {
            // 发送行动事件
            let action_type = match zone.action_type {
                ActionZoneType::BigBall => ActionType::BigBall,
                ActionZoneType::Shield => ActionType::Shield,
                ActionZoneType::MachineGun => ActionType::MachineGun,
                ActionZoneType::CIWS => ActionType::CIWS,
            };

            action_events.write(ActionEvent {
                team: marble.team,
                action_type,
                value: ((marble.value as f64 * zone.value_scale as f64) as u64)
                    .clamp(1, super::utils::MAX_VALUE),
            });

            // 重置弹珠
            marble.value = profile.marble.initial_value;

            if !zone.reset_position {
                continue;
            }

            // 找到对应颜色的起始点并重置位置
            for (spawn_point, spawn_transform) in spawn_points.iter() {
                if spawn_point.team == marble.team {
                    transform.translation = spawn_transform.translation;
                    // 重置速度
                    velocity.0 = spawn_velocity(&profile.marble);
                    break;
                }
            }
        }
    }
    profiler.add_counter(CounterId::PinballCollisionStartRead, read_events);
}

/// 防止弹珠离开弹珠机区域（安全检查）
pub fn contain_marbles(
    profiler: Res<Profiler>,
    profile: Res<PinballProfile>,
    mut marbles: Query<(&Marble, &mut Transform, &mut LinearVelocity)>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
) {
    let _scope = profiler.scope(ScopeId::PinballContainMarbles);
    let min_x = -profile.width / 2.0;
    let max_x = profile.width / 2.0;
    let min_y = -profile.height / 2.0;
    let max_y = profile.height / 2.0;

    for (marble, mut transform, mut velocity) in marbles.iter_mut() {
        let pos = transform.translation;

        // 如果弹珠离开区域，重置到起始点
        if pos.x < min_x - 50.0
            || pos.x > max_x + 50.0
            || pos.y < min_y - 50.0
            || pos.y > max_y + 50.0
        {
            for (spawn_point, spawn_transform) in spawn_points.iter() {
                if spawn_point.team == marble.team {
                    transform.translation = spawn_transform.translation;
                    velocity.0 = spawn_velocity(&profile.marble);
                    break;
                }
            }
        }
    }
}

/// 更新小球大小和文本
pub fn update_marble_display(
    profiler: Res<Profiler>,
    marbles: Query<(Entity, &Marble, &Transform), Changed<Marble>>,
    mut text_query: Query<(&MarbleText, &mut Text2d, &mut Transform), Without<Marble>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh_cache: ResMut<CircleMeshCache>,
) {
    let _scope = profiler.scope(ScopeId::PinballUpdateMarbleDisplay);
    for (marble_entity, marble, marble_transform) in marbles.iter() {
        let new_radius = calculate_radius(marble.value);
        let new_mesh = mesh_cache.circle(&mut *meshes, new_radius);

        // 更新小球 mesh
        commands.entity(marble_entity).insert(Mesh2d(new_mesh));

        // 更新碰撞体
        commands
            .entity(marble_entity)
            .insert(Collider::circle(new_radius));

        // 更新文本
        for (marker, mut text, mut text_transform) in text_query.iter_mut() {
            if marker.marble_entity == marble_entity {
                let value = format_value(marble.value);
                if text.0 != value {
                    text.0 = value;
                }
                text_transform.translation = marble_transform.translation.with_z(1.0);
                break;
            }
        }
    }
}

/// 同步小球和文本位置
pub fn sync_marble_text_position(
    profiler: Res<Profiler>,
    marbles: Query<(Entity, &Transform), (With<Marble>, Changed<Transform>)>,
    mut text_query: Query<(&MarbleText, &mut Transform), Without<Marble>>,
) {
    let _scope = profiler.scope(ScopeId::PinballSyncMarbleTextPosition);
    for (marble_entity, marble_transform) in marbles.iter() {
        for (marker, mut text_transform) in text_query.iter_mut() {
            if marker.marble_entity == marble_entity {
                text_transform.translation = marble_transform.translation.with_z(1.0);
            }
        }
    }
}

fn spawn_velocity(settings: &MarbleSettings) -> Vec2 {
    Vec2::new(
        rand::thread_rng().gen_range(-settings.spawn_speed..=settings.spawn_speed),
        0.0,
    )
}

pub fn check_boost_collision(
    mut collision_started: MessageReader<CollisionStart>,
    mut marbles: Query<&mut LinearVelocity, With<Marble>>,
    zones: Query<&BoostZone>,
) {
    for event in collision_started.read() {
        let pair = if marbles.contains(event.collider1) && zones.contains(event.collider2) {
            (event.collider1, event.collider2)
        } else if marbles.contains(event.collider2) && zones.contains(event.collider1) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };
        if let (Ok(mut velocity), Ok(zone)) = (marbles.get_mut(pair.0), zones.get(pair.1)) {
            velocity.0 += zone.velocity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collision_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Profiler>()
            .init_resource::<PinballProfile>()
            .add_message::<CollisionStart>()
            .add_message::<ActionEvent>()
            .add_systems(
                Update,
                (
                    cleanup_eliminated_marbles,
                    check_multiplier_collision,
                    check_action_zone_collision,
                    check_boost_collision,
                )
                    .chain(),
            );
        app.world_mut()
            .resource_mut::<PinballProfile>()
            .marble
            .initial_value = 77;
        app.world_mut()
            .resource_mut::<PinballProfile>()
            .marble
            .spawn_speed = 0.0;
        let marble = app
            .world_mut()
            .spawn((
                Marble {
                    team: TeamColor::Red,
                    value: 1000,
                },
                Transform::from_xyz(10.0, 20.0, 0.5),
                LinearVelocity(Vec2::ZERO),
            ))
            .id();
        let spawn = app
            .world_mut()
            .spawn((
                PinballSpawnPoint {
                    team: TeamColor::Red,
                },
                Transform::from_xyz(-60.0, 350.0, 0.5),
            ))
            .id();
        (app, marble, spawn)
    }

    fn collide(app: &mut App, marble: Entity, zone: Entity) {
        app.world_mut().write_message(CollisionStart {
            collider1: marble,
            collider2: zone,
            body1: Some(marble),
            body2: None,
        });
        app.update();
    }

    #[test]
    fn multiplier_and_boost_apply_profile_effects_without_teleport() {
        let (mut app, marble, _) = collision_app();
        let zone = app
            .world_mut()
            .spawn(MultiplierZone {
                multiplier: 7,
                reset_position: false,
            })
            .id();
        collide(&mut app, marble, zone);
        assert_eq!(app.world().get::<Marble>(marble).unwrap().value, 7000);
        assert_eq!(
            app.world().get::<Transform>(marble).unwrap().translation.x,
            10.0
        );
        let boost = app
            .world_mut()
            .spawn(BoostZone {
                velocity: Vec2::new(100.0, 200.0),
            })
            .id();
        collide(&mut app, marble, boost);
        assert_eq!(
            app.world().get::<LinearVelocity>(marble).unwrap().0,
            Vec2::new(100.0, 200.0)
        );
    }

    #[test]
    fn action_scales_output_and_uses_profile_initial_value_and_spawn() {
        let (mut app, marble, spawn) = collision_app();
        let zone = app
            .world_mut()
            .spawn(ActionZone {
                action_type: ActionZoneType::Shield,
                value_scale: 2.5,
                reset_position: true,
            })
            .id();
        collide(&mut app, marble, zone);
        let messages = app.world().resource::<Messages<ActionEvent>>();
        let mut cursor = messages.get_cursor();
        let action = cursor.read(messages).next().unwrap();
        assert_eq!(action.value, 2500);
        assert_eq!(action.action_type, ActionType::Shield);
        assert_eq!(app.world().get::<Marble>(marble).unwrap().value, 77);
        assert_eq!(
            app.world().get::<Transform>(marble).unwrap().translation,
            app.world().get::<Transform>(spawn).unwrap().translation
        );
        assert_eq!(
            app.world().get::<LinearVelocity>(marble).unwrap().0,
            Vec2::ZERO
        );
    }

    #[test]
    fn elimination_removes_marble_before_queued_actions_and_keeps_survivors_active() {
        let (mut app, eliminated_marble, _) = collision_app();
        let mut game_over = GameOver::default();
        game_over.eliminated[TeamColor::Red.index()] = true;
        app.insert_resource(game_over);
        let survivor = app
            .world_mut()
            .spawn((
                Marble {
                    team: TeamColor::Blue,
                    value: 1000,
                },
                Transform::from_xyz(10.0, 20.0, 0.5),
                LinearVelocity(Vec2::ZERO),
            ))
            .id();
        let spawn = app
            .world_mut()
            .spawn((
                PinballSpawnPoint {
                    team: TeamColor::Blue,
                },
                Transform::from_xyz(60.0, 350.0, 0.5),
            ))
            .id();
        let zone = app
            .world_mut()
            .spawn(ActionZone {
                action_type: ActionZoneType::Shield,
                value_scale: 2.5,
                reset_position: true,
            })
            .id();
        for marble in [eliminated_marble, survivor] {
            app.world_mut().write_message(CollisionStart {
                collider1: marble,
                collider2: zone,
                body1: Some(marble),
                body2: None,
            });
        }
        app.update();
        assert!(app.world().get_entity(eliminated_marble).is_err());
        assert_eq!(app.world().get::<Marble>(survivor).unwrap().value, 77);
        assert_eq!(
            app.world().get::<Transform>(survivor).unwrap().translation,
            app.world().get::<Transform>(spawn).unwrap().translation
        );
        let messages = app.world().resource::<Messages<ActionEvent>>();
        let mut cursor = messages.get_cursor();
        let actions: Vec<_> = cursor.read(messages).collect();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].team, TeamColor::Blue);
        assert_eq!(actions[0].value, 2500);
    }

    #[test]
    fn containment_uses_profile_dimensions() {
        let (mut app, marble, spawn) = collision_app();
        app.add_systems(Update, contain_marbles);
        app.world_mut().resource_mut::<PinballProfile>().width = 1000.0;
        app.world_mut()
            .get_mut::<Transform>(marble)
            .unwrap()
            .translation
            .x = 400.0;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(marble).unwrap().translation.x,
            400.0
        );
        app.world_mut()
            .get_mut::<Transform>(marble)
            .unwrap()
            .translation
            .x = 600.0;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(marble).unwrap().translation,
            app.world().get::<Transform>(spawn).unwrap().translation
        );
    }
}
