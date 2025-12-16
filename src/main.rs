mod colors;
mod events;
mod pinball;
mod territory;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::camera::{OrthographicProjection, Projection, ScalingMode, Viewport};
use bevy::camera::visibility::RenderLayers;

use events::{ActionEvent, UnitDestroyedEvent, VictoryEvent};
use pinball::PinballPlugin;
use territory::TerritoryPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "弹珠领土占领".to_string(),
                resolution: (1400, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec2::NEG_Y * 490.0)) // 重力加速度
        .add_message::<ActionEvent>()
        .add_message::<VictoryEvent>()
        .add_message::<UnitDestroyedEvent>()
        .add_plugins(PinballPlugin)
        .add_plugins(TerritoryPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, window: Single<&Window>) {
    let window_size = window.resolution.physical_size();
    let left_width = (window_size.x as f32 * (600.0 / 1400.0)).round() as u32;
    let right_width = window_size.x.saturating_sub(left_width);

    // 左侧：弹珠机相机
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            viewport: Some(Viewport {
                physical_position: UVec2::new(0, 0),
                physical_size: UVec2::new(left_width, window_size.y),
                ..default()
            }),
            ..default()
        },
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(-400.0, 0.0, 999.0),
        RenderLayers::layer(0),
    ));

    // 右侧：领土战场相机
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            viewport: Some(Viewport {
                physical_position: UVec2::new(left_width, 0),
                physical_size: UVec2::new(right_width, window_size.y),
                ..default()
            }),
            ..default()
        },
        Projection::from(OrthographicProjection {
            // 让战场的“游戏空间宽度”在任意窗口大小下保持固定，不与像素绑定
            scaling_mode: ScalingMode::FixedHorizontal {
                viewport_width: territory::TERRITORY_LOGIC_WIDTH,
            },
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 999.0),
        RenderLayers::layer(1),
    ));
}
