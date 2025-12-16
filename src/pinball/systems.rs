use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use rand::Rng;

use crate::colors::TeamColor;
use crate::events::{ActionEvent, ActionType};
use super::components::*;
use super::layout::{PINBALL_HEIGHT, PINBALL_WIDTH};
use super::utils::{calculate_radius, format_value};

const STUCK_TIME_SECS: f32 = 1.5;
const STUCK_MOVE_EPS: f32 = 0.8;
const STUCK_SPEED_EPS: f32 = 5.0;
const STUCK_LIFT_SPEED: f32 = 420.0;

/// 生成初始弹珠
pub fn spawn_initial_marbles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform)>,
    asset_server: Res<AssetServer>,
) {
    for (spawn_point, transform) in spawn_points.iter() {
        let material = materials.add(spawn_point.team.to_color());
        spawn_marble(
            &mut commands,
            &mut meshes,
            material,
            spawn_point.team,
            transform.translation.truncate(),
            &asset_server,
        );
    }
}

fn spawn_marble(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: Handle<ColorMaterial>,
    team: TeamColor,
    position: Vec2,
    asset_server: &Res<AssetServer>,
) {
    let mut rng = rand::thread_rng();
    // 给小球一个随机的初始水平速度
    let initial_velocity = Vec2::new(rng.gen_range(-50.0..50.0), 0.0);

    let marble = Marble::new(team);
    let radius = calculate_radius(marble.value);
    let mesh = meshes.add(Circle::new(radius));

    let marble_entity = commands.spawn((
        marble.clone(),
        RenderLayers::layer(0),
        StuckMarbleTracker { last_pos: position, still_time: 0.0 },
        RigidBody::Dynamic,
        Collider::circle(radius),
        Restitution::new(0.6), // 弹性
        Friction::new(0.3),
        LinearVelocity(initial_velocity),
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.5)),
        CollisionEventsEnabled,
    )).id();

    // 添加数值文本
    let text_value = format_value(marble.value);
    commands.spawn((
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
    time: Res<Time>,
    mut marbles: Query<(&Transform, &mut LinearVelocity, &mut StuckMarbleTracker), With<Marble>>,
) {
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
            velocity.0.y = velocity.0.y.max(STUCK_LIFT_SPEED);
            velocity.0.x += rng.gen_range(-60.0..60.0);
            tracker.still_time = 0.0;
            tracker.last_pos = pos;
        }
    }
}

/// 检测弹珠进入加倍区域
pub fn check_multiplier_collision(
    mut collision_started: MessageReader<CollisionStart>,
    mut marbles: Query<(&mut Marble, &mut Transform, &mut LinearVelocity)>,
    multiplier_zones: Query<&MultiplierZone>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
) {
    for event in collision_started.read() {
        // 检查是否是弹珠和加倍区碰撞
        let (marble_entity, zone_entity) = if marbles.contains(event.collider1) && multiplier_zones.contains(event.collider2) {
            (event.collider1, event.collider2)
        } else if marbles.contains(event.collider2) && multiplier_zones.contains(event.collider1) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };

        if let (Ok((mut marble, mut transform, mut velocity)), Ok(zone)) =
            (marbles.get_mut(marble_entity), multiplier_zones.get(zone_entity))
        {
            // 加倍
            marble.multiply(zone.multiplier);

            // 找到对应颜色的起始点并重置位置
            for (spawn_point, spawn_transform) in spawn_points.iter() {
                if spawn_point.team == marble.team {
                    transform.translation = spawn_transform.translation;
                    // 重置速度
                    let mut rng = rand::thread_rng();
                    velocity.0 = Vec2::new(rng.gen_range(-50.0..50.0), 0.0);
                    break;
                }
            }
        }
    }
}

/// 检测弹珠进入行动选择区域
pub fn check_action_zone_collision(
    mut collision_started: MessageReader<CollisionStart>,
    mut marbles: Query<(Entity, &mut Marble, &mut Transform, &mut LinearVelocity)>,
    action_zones: Query<&ActionZone>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
    mut action_events: MessageWriter<ActionEvent>,
) {
    for event in collision_started.read() {
        // 检查是否是弹珠和行动区碰撞
        let (marble_entity, zone_entity) = if marbles.contains(event.collider1) && action_zones.contains(event.collider2) {
            (event.collider1, event.collider2)
        } else if marbles.contains(event.collider2) && action_zones.contains(event.collider1) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };

        if let (Ok((_entity, mut marble, mut transform, mut velocity)), Ok(zone)) =
            (marbles.get_mut(marble_entity), action_zones.get(zone_entity))
        {
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
                value: marble.value,
            });

            // 重置弹珠
            marble.reset();

            // 找到对应颜色的起始点并重置位置
            for (spawn_point, spawn_transform) in spawn_points.iter() {
                if spawn_point.team == marble.team {
                    transform.translation = spawn_transform.translation;
                    // 重置速度
                    let mut rng = rand::thread_rng();
                    velocity.0 = Vec2::new(rng.gen_range(-50.0..50.0), 0.0);
                    break;
                }
            }
        }
    }
}

/// 防止弹珠离开弹珠机区域（安全检查）
pub fn contain_marbles(
    mut marbles: Query<(&Marble, &mut Transform, &mut LinearVelocity)>,
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
) {
    let min_x = -PINBALL_WIDTH / 2.0;
    let max_x = PINBALL_WIDTH / 2.0;
    let min_y = -PINBALL_HEIGHT / 2.0;
    let max_y = PINBALL_HEIGHT / 2.0;

    for (marble, mut transform, mut velocity) in marbles.iter_mut() {
        let pos = transform.translation;

        // 如果弹珠离开区域，重置到起始点
        if pos.x < min_x - 50.0 || pos.x > max_x + 50.0 ||
           pos.y < min_y - 50.0 || pos.y > max_y + 50.0
        {
            for (spawn_point, spawn_transform) in spawn_points.iter() {
                if spawn_point.team == marble.team {
                    transform.translation = spawn_transform.translation;
                    let mut rng = rand::thread_rng();
                    velocity.0 = Vec2::new(rng.gen_range(-50.0..50.0), 0.0);
                    break;
                }
            }
        }
    }
}

/// 更新小球大小和文本
pub fn update_marble_display(
    marbles: Query<(Entity, &Marble, &Transform), Changed<Marble>>,
    mut text_query: Query<(&MarbleText, &mut Text2d, &mut Transform), Without<Marble>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (marble_entity, marble, marble_transform) in marbles.iter() {
        let new_radius = calculate_radius(marble.value);
        let new_mesh = meshes.add(Circle::new(new_radius));

        // 更新小球 mesh
        commands.entity(marble_entity).insert(Mesh2d(new_mesh));

        // 更新碰撞体
        commands.entity(marble_entity).insert(Collider::circle(new_radius));

        // 更新文本
        for (marker, mut text, mut text_transform) in text_query.iter_mut() {
            if marker.marble_entity == marble_entity {
                text.0 = format_value(marble.value);
                text_transform.translation = marble_transform.translation.with_z(1.0);
                break;
            }
        }
    }
}

/// 同步小球和文本位置
pub fn sync_marble_text_position(
    marbles: Query<(Entity, &Transform), (With<Marble>, Changed<Transform>)>,
    mut text_query: Query<(&MarbleText, &mut Transform), Without<Marble>>,
) {
    for (marble_entity, marble_transform) in marbles.iter() {
        for (marker, mut text_transform) in text_query.iter_mut() {
            if marker.marble_entity == marble_entity {
                text_transform.translation = marble_transform.translation.with_z(1.0);
            }
        }
    }
}
