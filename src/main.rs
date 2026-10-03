use avian2d::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{OrthographicProjection, Projection, RenderTarget, ScalingMode, Viewport};
use bevy::diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::time::Virtual;
use bevy::window::PrimaryWindow;
use std::time::Duration;

use bevymarble::events::{ActionEvent, UnitDestroyedEvent, VictoryEvent};
use bevymarble::pinball::PinballPlugin;
use bevymarble::pinball::profile::{PinballProfile, ProfileLibrary, profile_path};
use bevymarble::profiler::ProfilerPlugin;
use bevymarble::territory::{self, TerritoryPlugin};

#[cfg(test)]
mod layout_tests;

const PINBALL_VIEWPORT_FRACTION: f32 = 600.0 / 1400.0;
const PINBALL_CAMERA_PADDING: f32 = 40.0;
const TERRITORY_CAMERA_PADDING: f32 = 48.0;

#[derive(Component)]
struct GameCamera;

fn main() -> Result<(), String> {
    let profile = ProfileLibrary::load(&profile_path())?.active().clone();
    let gravity = Gravity(Vec2::from_array(profile.gravity));
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "弹珠领土占领".to_string(),
                resolution: (1400, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(EntityCountDiagnosticsPlugin::default())
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(gravity)
        .insert_resource(profile)
        .add_message::<ActionEvent>()
        .add_message::<VictoryEvent>()
        .add_message::<UnitDestroyedEvent>()
        .add_plugins(ProfilerPlugin)
        .add_plugins(PinballPlugin)
        .add_plugins(TerritoryPlugin)
        .add_systems(Startup, (configure_time, setup))
        .add_systems(Update, update_game_viewports)
        .run();
    Ok(())
}

fn configure_time(mut time: ResMut<Time<Virtual>>) {
    // 限制“追帧尖峰”：当一帧非常慢时，FixedUpdate 不会在单帧内无限补跑。
    // 代价是过载时游戏时间会变慢，但能避免死亡螺旋（卡顿越卡越卡）。
    time.set_max_delta(Duration::from_millis(50));
}

fn setup(
    mut commands: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    profile: Res<PinballProfile>,
) {
    let window_size = window.resolution.physical_size();
    let left_width = (window_size.x as f32 * PINBALL_VIEWPORT_FRACTION).round() as u32;
    let right_width = window_size.x.saturating_sub(left_width);

    // 左侧：弹珠机相机
    commands.spawn((
        GameCamera,
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
            // 两个轴都容纳完整场地，并在外框周围留出显示空间。
            scaling_mode: ScalingMode::AutoMin {
                min_width: profile.width + PINBALL_CAMERA_PADDING * 2.0,
                min_height: profile.height + PINBALL_CAMERA_PADDING * 2.0,
            },
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 999.0),
        RenderLayers::layer(0),
    ));

    // 右侧：领土战场相机
    commands.spawn((
        GameCamera,
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
            scaling_mode: ScalingMode::AutoMin {
                min_width: territory::TERRITORY_LOGIC_WIDTH + TERRITORY_CAMERA_PADDING * 2.0,
                min_height: territory::TERRITORY_LOGIC_WIDTH + TERRITORY_CAMERA_PADDING * 2.0,
            },
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 999.0),
        RenderLayers::layer(1),
    ));
}

fn update_game_viewports(
    window: Single<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<GameCamera>>,
) {
    let size = window.resolution.physical_size();
    let left_width = (size.x as f32 * PINBALL_VIEWPORT_FRACTION).round() as u32;
    for mut camera in &mut cameras {
        let (position, viewport_size) = if camera.order == 0 {
            (UVec2::ZERO, UVec2::new(left_width, size.y))
        } else {
            (
                UVec2::new(left_width, 0),
                UVec2::new(size.x - left_width, size.y),
            )
        };
        let active = viewport_size.min_element() > 0;
        if camera.is_active != active {
            camera.is_active = active;
        }
        if !active {
            continue;
        }
        // DPI 变化的这一帧由 Bevy 缩放视口，随后按窗口实际尺寸核对。
        // 避免新视口再次被引擎按 DPI 比例放大。
        if matches!(camera.target, RenderTarget::Window(_))
            && camera
                .target_scaling_factor()
                .is_some_and(|scale| scale != window.scale_factor())
        {
            continue;
        }
        if camera.viewport.as_ref().is_some_and(|viewport| {
            viewport.physical_position == position && viewport.physical_size == viewport_size
        }) {
            continue;
        }
        camera.viewport = Some(Viewport {
            physical_position: position,
            physical_size: viewport_size,
            ..default()
        });
    }
}
