use bevy::prelude::*;

use crate::colors::TeamColor;
use super::coords::logic_to_render;
use super::grid::TerritoryGrid;

/// 开局初始化系统
pub fn setup_initial_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    grid: Res<TerritoryGrid>,
) {
    // 每个队伍开局分配等量的机关枪和近防炮
    for team in TeamColor::all() {
        let (start_x, start_y) = team.start_corner();
        let corner_logic = grid.grid_to_logic(start_x, start_y);

        // 往中心方向偏移，确保在边界内
        let offset = Vec2::new(
            if start_x == 0 { 50.0 } else { -50.0 },
            if start_y == 0 { 50.0 } else { -50.0 },
        );
        let base_logic = corner_logic + offset;
        let base_render = logic_to_render(base_logic);

        // HQ - 在角落内侧
        spawn_hq(&mut commands, &mut meshes, &mut materials, team, base_render);

        // 初始机关枪 - 2个，在HQ中心
        spawn_initial_machine_gun(&mut commands, &mut meshes, &mut materials, team, base_render);
        spawn_initial_machine_gun(&mut commands, &mut meshes, &mut materials, team, base_render);

        // 初始近防炮 - 1个，在HQ中心
        spawn_initial_ciws(&mut commands, &mut meshes, &mut materials, team, base_render);
    }
}

fn spawn_hq(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    team: TeamColor,
    position: Vec2,
) {
    use super::components::HQ;

    let mesh = meshes.add(Rectangle::new(30.0, 30.0));
    let mut color = team.to_color();
    color.set_alpha(0.8);
    let material = materials.add(color);

    commands.spawn((
        HQ { team },
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(2.0)),
    ));
}

fn spawn_initial_machine_gun(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    team: TeamColor,
    position: Vec2,
) {
    use super::components::{MachineGun, TerritoryUnit};
    use std::f32::consts::PI;

    let mesh = meshes.add(Rectangle::new(15.0, 15.0));
    let material = materials.add(team.to_color());

    commands.spawn((
        MachineGun {
            team,
            bullets: 10_000_000, // 初始10M子弹
            fire_timer: Timer::from_seconds(0.001, TimerMode::Repeating), // 每秒100发
            rotation: 0.0,
            rotation_speed: PI * 2.0, // 旋转
        },
        TerritoryUnit { team },
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.9)),
    ));
}

fn spawn_initial_ciws(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    team: TeamColor,
    position: Vec2,
) {
    use super::components::{CIWS, TerritoryUnit};

    let mesh = meshes.add(Circle::new(10.0));
    let material = materials.add(team.to_color());

    commands.spawn((
        CIWS {
            team,
            bullets: 10_000_000, // 初始10M子弹
            fire_timer: Timer::from_seconds(0.3, TimerMode::Repeating),
        },
        TerritoryUnit { team },
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.9)),
    ));
}
