use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

use crate::colors::TeamColor;
use super::components::*;

/// 弹珠机布局常量
pub const PINBALL_WIDTH: f32 = 400.0;
pub const PINBALL_HEIGHT: f32 = 800.0;
pub const PINBALL_OFFSET_X: f32 = -400.0; // 弹珠机在左侧
pub const WALL_THICKNESS: f32 = 10.0;
pub const PEG_RADIUS: f32 = 8.0;
pub const ZONE_WIDTH: f32 = 80.0;
pub const ZONE_HEIGHT: f32 = 40.0;

/// 生成弹珠机布局
pub fn spawn_pinball_layout(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let wall_color = materials.add(Color::srgb(0.3, 0.3, 0.4));
    let peg_color = materials.add(Color::srgb(0.5, 0.5, 0.6));

    let ui_font: Handle<Font> = asset_server.load("fonts/FiraSans-Bold.ttf");

    // 左边界墙
    spawn_wall(
        &mut commands,
        &mut meshes,
        wall_color.clone(),
        Vec2::new(PINBALL_OFFSET_X - PINBALL_WIDTH / 2.0, 0.0),
        Vec2::new(WALL_THICKNESS, PINBALL_HEIGHT),
    );

    // 右边界墙
    spawn_wall(
        &mut commands,
        &mut meshes,
        wall_color.clone(),
        Vec2::new(PINBALL_OFFSET_X + PINBALL_WIDTH / 2.0, 0.0),
        Vec2::new(WALL_THICKNESS, PINBALL_HEIGHT),
    );

    // 底部墙（带斜坡引导到选择区）
    spawn_wall(
        &mut commands,
        &mut meshes,
        wall_color.clone(),
        Vec2::new(PINBALL_OFFSET_X, -PINBALL_HEIGHT / 2.0),
        Vec2::new(PINBALL_WIDTH, WALL_THICKNESS),
    );

    // 顶部墙
    spawn_wall(
        &mut commands,
        &mut meshes,
        wall_color.clone(),
        Vec2::new(PINBALL_OFFSET_X, PINBALL_HEIGHT / 2.0),
        Vec2::new(PINBALL_WIDTH, WALL_THICKNESS),
    );

    // 生成钉子布局（固定设计）
    spawn_pegs(&mut commands, &mut meshes, peg_color, &mut materials);

    // 生成加倍区域
    spawn_multiplier_zones(&mut commands, &mut meshes, &mut materials, &ui_font);

    // 生成行动选择区域
    spawn_action_zones(&mut commands, &mut meshes, &mut materials, &ui_font);

    // 生成四个颜色的小球起始点
    spawn_marble_spawn_points(&mut commands);
}

fn spawn_zone_label(
    commands: &mut Commands,
    text: impl Into<String>,
    position: Vec3,
    font: &Handle<Font>,
    font_size: f32,
) {
    commands.spawn((
        Text2d::new(text),
        RenderLayers::layer(0),
        TextFont {
            font: font.clone(),
            font_size,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_translation(position),
    ));
}

fn spawn_wall(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: Handle<ColorMaterial>,
    position: Vec2,
    size: Vec2,
) {
    commands.spawn((
        PinballWall,
        RenderLayers::layer(0),
        RigidBody::Static,
        Collider::rectangle(size.x, size.y),
        Mesh2d(meshes.add(Rectangle::new(size.x, size.y))),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.0)),
    ));
}

fn spawn_pegs(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    peg_material: Handle<ColorMaterial>,
    _materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    // 固定钉子布局 - 多行交错排列，位置更合理
    let rows = [
        // 上层钉子
        (250.0, vec![-120.0, -40.0, 40.0, 120.0]),
        (200.0, vec![-80.0, 0.0, 80.0]),
        (150.0, vec![-120.0, -40.0, 40.0, 120.0]),
        // 中层钉子（加倍区周围）
        (50.0, vec![-140.0, -60.0, 60.0, 140.0]),
        (0.0, vec![-100.0, -20.0, 20.0, 100.0]),
        // 下层钉子
        (-100.0, vec![-140.0, -60.0, 60.0, 140.0]),
        (-150.0, vec![-100.0, -20.0, 20.0, 100.0]),
        (-200.0, vec![-140.0, -60.0, 60.0, 140.0]),
    ];

    let mesh = meshes.add(Circle::new(PEG_RADIUS));

    for (y, x_positions) in rows.iter() {
        for x in x_positions.iter() {
            commands.spawn((
                PinballPeg,
                RenderLayers::layer(0),
                RigidBody::Static,
                Collider::circle(PEG_RADIUS),
                Mesh2d(mesh.clone()),
                MeshMaterial2d(peg_material.clone()),
                Transform::from_translation(Vec3::new(
                    PINBALL_OFFSET_X + *x,
                    *y,
                    0.0,
                )),
            ));
        }
    }
}

fn spawn_multiplier_zones(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    ui_font: &Handle<Font>,
) {
    let zone_mesh = meshes.add(Rectangle::new(ZONE_WIDTH, ZONE_HEIGHT));

    // x2 区域（两个，在中层两侧）
    let x2_color = materials.add(Color::srgba(0.2, 0.8, 0.2, 0.5));
    for x_offset in [-100.0, 100.0] {
        let zone_center = Vec3::new(PINBALL_OFFSET_X + x_offset, -50.0, 0.2);
        commands.spawn((
            MultiplierZone { multiplier: 2 },
            RenderLayers::layer(0),
            Collider::rectangle(ZONE_WIDTH, ZONE_HEIGHT),
            Sensor,
            Mesh2d(zone_mesh.clone()),
            MeshMaterial2d(x2_color.clone()),
            Transform::from_translation(Vec3::new(
                PINBALL_OFFSET_X + x_offset,
                -50.0,  // 降低位置
                0.1,
            )),
        ));
        spawn_zone_label(commands, "x2", zone_center, ui_font, 18.0);
    }

    // x4 区域（中间，更低）
    let x4_color = materials.add(Color::srgba(0.8, 0.6, 0.2, 0.5));
    let x4_center = Vec3::new(PINBALL_OFFSET_X, -50.0, 0.2);
    commands.spawn((
        MultiplierZone { multiplier: 4 },
        RenderLayers::layer(0),
        Collider::rectangle(ZONE_WIDTH, ZONE_HEIGHT),
        Sensor,
        Mesh2d(zone_mesh.clone()),
        MeshMaterial2d(x4_color),
        Transform::from_translation(Vec3::new(PINBALL_OFFSET_X, -50.0, 0.1)),
    ));
    spawn_zone_label(commands, "x4", x4_center, ui_font, 18.0);

    // x8 区域（最难到达，在上层中间小区域）
    let x8_color = materials.add(Color::srgba(0.9, 0.2, 0.8, 0.5));
    let x8_mesh = meshes.add(Rectangle::new(ZONE_WIDTH * 0.6, ZONE_HEIGHT));
    let x8_center = Vec3::new(PINBALL_OFFSET_X, 100.0, 0.2);
    commands.spawn((
        MultiplierZone { multiplier: 8 },
        RenderLayers::layer(0),
        Collider::rectangle(ZONE_WIDTH * 0.6, ZONE_HEIGHT),
        Sensor,
        Mesh2d(x8_mesh),
        MeshMaterial2d(x8_color),
        Transform::from_translation(Vec3::new(PINBALL_OFFSET_X, 100.0, 0.1)),  // 降低位置
    ));
    spawn_zone_label(commands, "x8", x8_center, ui_font, 18.0);

    // 在加倍区上方添加小格点，阻止超大球进入
    let barrier_peg_color = materials.add(Color::srgb(0.7, 0.3, 0.3));
    spawn_barrier_pegs(commands, meshes, barrier_peg_color);
}

/// 在加倍区上方添加阻挡格点
fn spawn_barrier_pegs(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    peg_material: Handle<ColorMaterial>,
) {
    let barrier_y = -20.0; // 加倍区上方
    let barrier_positions = vec![
        -140.0, -100.0, -60.0, -20.0, 20.0, 60.0, 100.0, 140.0,
    ];

    let mesh = meshes.add(Circle::new(PEG_RADIUS));

    for x in barrier_positions {
        commands.spawn((
            PinballPeg,
            RenderLayers::layer(0),
            RigidBody::Static,
            Collider::circle(PEG_RADIUS),
            Mesh2d(mesh.clone()),
            MeshMaterial2d(peg_material.clone()),
            Transform::from_translation(Vec3::new(
                PINBALL_OFFSET_X + x,
                barrier_y,
                0.0,
            )),
        ));
    }
}

fn spawn_action_zones(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    ui_font: &Handle<Font>,
) {
    // 完全覆盖底部，避免缝隙
    let total_width = PINBALL_WIDTH - WALL_THICKNESS * 2.0;
    let zone_width = total_width / 4.0;
    let zone_height = ZONE_HEIGHT * 2.0;
    let zone_mesh = meshes.add(Rectangle::new(zone_width, zone_height));

    let zones = [
        (ActionZoneType::BigBall, -1.5 * zone_width, Color::srgba(0.9, 0.3, 0.3, 0.6)),
        (ActionZoneType::Shield, -0.5 * zone_width, Color::srgba(0.3, 0.5, 0.9, 0.6)),
        (ActionZoneType::MachineGun, 0.5 * zone_width, Color::srgba(0.9, 0.9, 0.3, 0.6)),
        (ActionZoneType::CIWS, 1.5 * zone_width, Color::srgba(0.9, 0.5, 0.2, 0.6)),
    ];

    let bottom_y = -PINBALL_HEIGHT / 2.0 + zone_height / 2.0 + WALL_THICKNESS;

    for (action_type, x_offset, color) in zones {
        let material = materials.add(color);
        let label = match action_type {
            ActionZoneType::BigBall => "Big Ball",
            ActionZoneType::Shield => "Shield",
            ActionZoneType::MachineGun => "MG",
            ActionZoneType::CIWS => "CIWS",
        };
        let zone_center = Vec3::new(PINBALL_OFFSET_X + x_offset, bottom_y, 0.2);
        commands.spawn((
            ActionZone { action_type },
            RenderLayers::layer(0),
            Collider::rectangle(zone_width, zone_height),
            Sensor,
            Mesh2d(zone_mesh.clone()),
            MeshMaterial2d(material),
            Transform::from_translation(Vec3::new(
                PINBALL_OFFSET_X + x_offset,
                bottom_y,
                0.1,
            )),
        ));
        spawn_zone_label(commands, label, zone_center, ui_font, 16.0);
    }
}

fn spawn_marble_spawn_points(commands: &mut Commands) {
    // 四个颜色的小球从顶部不同位置生成
    let spawn_positions = [
        (TeamColor::Red, -60.0),
        (TeamColor::Blue, -20.0),
        (TeamColor::Green, 20.0),
        (TeamColor::Yellow, 60.0),
    ];

    for (team, x_offset) in spawn_positions {
        commands.spawn((
            PinballSpawnPoint { team },
            RenderLayers::layer(0),
            Transform::from_translation(Vec3::new(
                PINBALL_OFFSET_X + x_offset,
                PINBALL_HEIGHT / 2.0 - 50.0,
                0.0,
            )),
        ));
    }
}
