use avian2d::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use super::components::*;
use super::profile::{ObjectKind, ObjectShape, PinballProfile};

// 默认布局尺寸，供性能覆盖层等使用；游戏尺寸从 profile 读取。
pub const PINBALL_WIDTH: f32 = 400.0;
pub const PINBALL_HEIGHT: f32 = 800.0;

pub fn spawn_pinball_layout(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
    profile: Res<PinballProfile>,
) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    for object in profile.objects.iter().filter(|object| object.enabled) {
        let position = Vec2::from_array(object.position);
        if let ObjectKind::Spawn { team } = object.kind {
            commands.spawn((
                PinballSceneEntity,
                PinballSpawnPoint { team },
                Transform::from_translation(position.extend(0.5)),
            ));
            continue;
        }
        // 将尺寸和缩放烘焙到网格、碰撞体，确保两者完全一致。
        let size = object.dimensions();
        let (mesh, collider) = match object.shape {
            ObjectShape::Rectangle => (
                meshes.add(Rectangle::new(size.x, size.y)),
                Collider::rectangle(size.x, size.y),
            ),
            ObjectShape::Ellipse => (
                meshes.add(Ellipse::new(size.x / 2.0, size.y / 2.0)),
                if size.x == size.y {
                    Collider::circle(size.x / 2.0)
                } else {
                    Collider::ellipse(size.x / 2.0, size.y / 2.0)
                },
            ),
        };
        let [r, g, b, a] = object.color;
        let mut entity = commands.spawn((
            PinballSceneEntity,
            Name::new(object.name.clone()),
            RenderLayers::layer(0),
            collider,
            Mesh2d(mesh),
            MeshMaterial2d(materials.add(Color::srgba(r, g, b, a))),
            Transform::from_translation(position.extend(if object.kind.is_solid() {
                0.0
            } else {
                0.1
            }))
            .with_rotation(Quat::from_rotation_z(object.rotation.to_radians())),
        ));
        match object.kind {
            ObjectKind::Wall | ObjectKind::Peg => {
                entity.insert((
                    RigidBody::Static,
                    Restitution::new(object.restitution),
                    Friction::new(object.friction),
                ));
                if matches!(object.kind, ObjectKind::Wall) {
                    entity.insert(PinballWall);
                } else {
                    entity.insert(PinballPeg);
                }
            }
            ObjectKind::Multiplier {
                factor,
                reset_position,
            } => {
                entity.insert((
                    Sensor,
                    MultiplierZone {
                        multiplier: factor,
                        reset_position,
                    },
                ));
            }
            ObjectKind::Action {
                action,
                value_scale,
                reset_position,
            } => {
                entity.insert((
                    Sensor,
                    ActionZone {
                        action_type: action,
                        value_scale,
                        reset_position,
                    },
                ));
            }
            ObjectKind::Boost { velocity } => {
                entity.insert((
                    Sensor,
                    BoostZone {
                        velocity: Vec2::from_array(velocity),
                    },
                ));
            }
            ObjectKind::Spawn { .. } => unreachable!(),
        }
        if !object.kind.is_solid() {
            entity.with_children(|parent| {
                parent.spawn((
                    Text2d::new(object.kind.label()),
                    RenderLayers::layer(0),
                    TextFont {
                        font: font.clone(),
                        font_size: if matches!(object.kind, ObjectKind::Multiplier { .. }) {
                            18.0
                        } else {
                            16.0
                        },
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, 0.0, 0.1),
                ));
            });
        }
    }
}
