mod colors;
mod events;
mod pinball;
mod territory;

use avian2d::prelude::*;
use bevy::prelude::*;

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

fn setup(mut commands: Commands) {
    // 生成 2D 相机
    commands.spawn(Camera2d);
}
