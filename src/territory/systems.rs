use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::time::Virtual;
use rand::Rng;

use crate::colors::TeamColor;
use crate::events::{ActionEvent, ActionType, UnitDestroyedEvent, VictoryEvent};
use crate::pinball::format_value;
use crate::profiler::{CounterId, Profiler, ScopeId};
use crate::territory::{CiwsDistanceMetric, GameOver, TerritorySettings};
use super::components::*;
use super::coords::{TERRITORY_LOGIC_HEIGHT, TERRITORY_LOGIC_WIDTH};
use super::grid::{ShieldInfo, TerritoryGrid};

const BIGBALL_RADIUS: f32 = 15.0;
const BIGBALL_SPEED: f32 = 100.0;
const BULLET_RADIUS: f32 = 6.0;
const BULLET_WIDTH: f32 = BULLET_RADIUS * 2.0;
const BULLET_LENGTH: f32 = BULLET_RADIUS * 2.6;
const BULLET_BOUND_HALF: f32 = BULLET_LENGTH / 2.0;
const BULLET_SPEED: f32 = 250.0;
const BULLET_MIN_SPEED: f32 = 100.0;
const SHIELD_RADIUS: f32 = 50.0;
const HQ_HALF_SIZE: f32 = 15.0;

#[derive(Resource, Clone)]
pub struct TerritoryRenderAssets {
    bullet_mesh: Handle<Mesh>,
    bigball_mesh: Handle<Mesh>,
    shield_mesh: Handle<Mesh>,
    machine_gun_mesh: Handle<Mesh>,
    ciws_mesh: Handle<Mesh>,
    team_materials: [Handle<ColorMaterial>; 4],
    shield_materials: [Handle<ColorMaterial>; 4],
}

impl TerritoryRenderAssets {
    fn team_material(&self, team: TeamColor) -> Handle<ColorMaterial> {
        self.team_materials[team.index()].clone()
    }

    fn shield_material(&self, team: TeamColor) -> Handle<ColorMaterial> {
        self.shield_materials[team.index()].clone()
    }
}

#[derive(Resource, Clone)]
pub struct TerritoryUiAssets {
    pub ui_font: Handle<Font>,
}

pub fn setup_territory_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let teams = TeamColor::all();
    let team_materials = std::array::from_fn(|i| materials.add(teams[i].to_color()));
    let shield_materials = std::array::from_fn(|i| {
        let mut color = teams[i].to_color();
        color.set_alpha(0.3);
        materials.add(color)
    });

    let bullet_mesh = meshes.add(Triangle2d::new(
        Vec2::new(BULLET_LENGTH / 2.0, 0.0),
        Vec2::new(-BULLET_LENGTH / 2.0, BULLET_WIDTH / 2.0),
        Vec2::new(-BULLET_LENGTH / 2.0, -BULLET_WIDTH / 2.0),
    ));

    commands.insert_resource(TerritoryRenderAssets {
        bullet_mesh,
        bigball_mesh: meshes.add(Circle::new(BIGBALL_RADIUS)),
        shield_mesh: meshes.add(Circle::new(SHIELD_RADIUS)),
        machine_gun_mesh: meshes.add(Rectangle::new(15.0, 15.0)),
        ciws_mesh: meshes.add(Circle::new(10.0)),
        team_materials,
        shield_materials,
    });

    commands.insert_resource(TerritoryUiAssets {
        ui_font: asset_server.load("fonts/FiraSans-Bold.ttf"),
    });
}

#[derive(Resource, Debug, Clone)]
pub struct BulletPaintKernel {
    pub row_spans: Vec<(i32, i32)>, // (dy, dx_max)
    pub points_est: u64,
}

impl Default for BulletPaintKernel {
    fn default() -> Self {
        // 将子弹半径（游戏空间单位）换算为“格子半径”并预计算圆形覆盖 offset。
        // 这里不依赖任何渲染尺寸；只要 grid 的逻辑宽度不变，覆盖规则就稳定。
        let cells_per_unit = 1024.0 / TERRITORY_LOGIC_WIDTH;
        let radius = (BULLET_RADIUS * cells_per_unit).ceil().max(1.0) as i32;

        let mut row_spans = Vec::new();
        let mut points_est = 0u64;
        for dy in -radius..=radius {
            let dx_max = (((radius * radius - dy * dy) as f64).sqrt().floor() as i32).max(0);
            row_spans.push((dy, dx_max));
            points_est += (dx_max as i64 * 2 + 1).max(0) as u64;
        }

        Self { row_spans, points_est }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct BigBallPaintKernel {
    pub offsets: Vec<(i32, i32)>,
    pub radius: i32,
}

impl Default for BigBallPaintKernel {
    fn default() -> Self {
        let cells_per_unit = 1024.0 / TERRITORY_LOGIC_WIDTH;
        // 保持与原先 `as i32` 的行为一致（向 0 取整）
        let radius = (BIGBALL_RADIUS * cells_per_unit).max(1.0) as i32;

        let mut offsets = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    offsets.push((dx, dy));
                }
            }
        }

        Self { offsets, radius }
    }
}

#[derive(Debug, Clone, Copy)]
struct TargetEntry {
    entity: Entity,
    team: TeamColor,
    pos: Vec2,
}

#[derive(Resource, Debug)]
pub struct TargetSpatialIndex {
    cell_size: f32,
    grid_w: i32,
    grid_h: i32,
    min_x: f32,
    min_y: f32,
    buckets: Vec<Vec<TargetEntry>>,
}

impl Default for TargetSpatialIndex {
    fn default() -> Self {
        let cell_size = 50.0;
        let min_x = -TERRITORY_LOGIC_WIDTH / 2.0;
        let min_y = -TERRITORY_LOGIC_HEIGHT / 2.0;
        let grid_w = (TERRITORY_LOGIC_WIDTH / cell_size).ceil() as i32;
        let grid_h = (TERRITORY_LOGIC_HEIGHT / cell_size).ceil() as i32;
        let bucket_count = (grid_w * grid_h).max(0) as usize;
        Self {
            cell_size,
            grid_w,
            grid_h,
            min_x,
            min_y,
            buckets: vec![Vec::new(); bucket_count],
        }
    }
}

impl TargetSpatialIndex {
    fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
    }

    fn cell_of(&self, pos: Vec2) -> Option<(i32, i32)> {
        let x = ((pos.x - self.min_x) / self.cell_size).floor() as i32;
        let y = ((pos.y - self.min_y) / self.cell_size).floor() as i32;
        if x < 0 || y < 0 || x >= self.grid_w || y >= self.grid_h {
            return None;
        }
        Some((x, y))
    }

    fn bucket_index(&self, cell_x: i32, cell_y: i32) -> usize {
        (cell_y * self.grid_w + cell_x) as usize
    }

    fn cell_aabb(&self, cell_x: i32, cell_y: i32) -> (f32, f32, f32, f32) {
        let left = self.min_x + cell_x as f32 * self.cell_size;
        let bottom = self.min_y + cell_y as f32 * self.cell_size;
        (left, bottom, left + self.cell_size, bottom + self.cell_size)
    }

    fn nearest_enemy_pos(&self, origin: Vec2, team: TeamColor, metric: CiwsDistanceMetric) -> Option<Vec2> {
        let (cx, cy) = self.cell_of(origin)?;

        let mut best: Option<(u64, f32, Vec2)> = None; // (entity_bits, score, pos)
        let mut r = 0;
        let max_r = self.grid_w.max(self.grid_h);
        let eps = 1e-6_f32;

        while r <= max_r {
            let min_x = (cx - r).max(0);
            let max_x = (cx + r).min(self.grid_w - 1);
            let min_y = (cy - r).max(0);
            let max_y = (cy + r).min(self.grid_h - 1);

            let visit_cell = |x: i32, y: i32, best: &mut Option<(u64, f32, Vec2)>| {
                let idx = self.bucket_index(x, y);
                for entry in &self.buckets[idx] {
                    if entry.team == team {
                        continue;
                    }
                    let bits = entry.entity.to_bits();
                    let score = match metric {
                        CiwsDistanceMetric::Manhattan => {
                            (origin.x - entry.pos.x).abs() + (origin.y - entry.pos.y).abs()
                        }
                        CiwsDistanceMetric::EuclideanSquared => origin.distance_squared(entry.pos),
                    };
                    match best {
                        None => *best = Some((bits, score, entry.pos)),
                        Some((best_bits, best_score, _)) => {
                            if score + eps < *best_score || ((score - *best_score).abs() <= eps && bits < *best_bits) {
                                *best = Some((bits, score, entry.pos));
                            }
                        }
                    }
                }
            };

            if r == 0 {
                visit_cell(cx, cy, &mut best);
            } else {
                for x in min_x..=max_x {
                    visit_cell(x, min_y, &mut best);
                    if min_y != max_y {
                        visit_cell(x, max_y, &mut best);
                    }
                }
                for y in (min_y + 1)..=(max_y - 1) {
                    visit_cell(min_x, y, &mut best);
                    if min_x != max_x {
                        visit_cell(max_x, y, &mut best);
                    }
                }
            }

            if let Some((_, best_score, _)) = best {
                // 计算“离开当前已搜索正方形”的最小距离，作为外层环的下界
                let (left, bottom, right, top) = {
                    let (l0, b0, _, _) = self.cell_aabb(min_x, min_y);
                    let (_, _, r1, t1) = self.cell_aabb(max_x, max_y);
                    (l0, b0, r1, t1)
                };

                if origin.x >= left && origin.x <= right && origin.y >= bottom && origin.y <= top {
                    let to_left = origin.x - left;
                    let to_right = right - origin.x;
                    let to_bottom = origin.y - bottom;
                    let to_top = top - origin.y;
                    let boundary = to_left.min(to_right).min(to_bottom.min(to_top));
                    match metric {
                        CiwsDistanceMetric::Manhattan => {
                            if boundary > best_score {
                                break;
                            }
                        }
                        CiwsDistanceMetric::EuclideanSquared => {
                            if boundary * boundary > best_score {
                                break;
                            }
                        }
                    }
                }
            }

            r += 1;
        }

        best.map(|(_, _, pos)| pos)
    }
}

/// 为 CIWS 构建目标的空间索引（稳定 tie-break：同距离时选 Entity bits 更小的）
pub fn update_target_spatial_index(
    profiler: Res<Profiler>,
    mut index: ResMut<TargetSpatialIndex>,
    settings: Res<TerritorySettings>,
    bigballs: Query<(Entity, &TerritoryUnit, &Transform), With<BigBall>>,
    bullets: Query<(Entity, &Bullet, &Transform), With<Bullet>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryUpdateTargetSpatialIndex);
    index.clear();
    for (entity, unit, transform) in bigballs.iter() {
        let pos = transform.translation.truncate();
        if let Some((x, y)) = index.cell_of(pos) {
            let idx = index.bucket_index(x, y);
            index.buckets[idx].push(TargetEntry {
                entity,
                team: unit.team,
                pos,
            });
        }
    }

    if !settings.ciws_target_bullets {
        return;
    }

    for (entity, bullet, transform) in bullets.iter() {
        let pos = transform.translation.truncate();
        if let Some((x, y)) = index.cell_of(pos) {
            let idx = index.bucket_index(x, y);
            index.buckets[idx].push(TargetEntry {
                entity,
                team: bullet.team,
                pos,
            });
        }
    }
}

/// 响应行动事件，生成对应单位
pub fn spawn_units_from_events(
    profiler: Res<Profiler>,
    mut commands: Commands,
    mut events: MessageReader<ActionEvent>,
    render_assets: Res<TerritoryRenderAssets>,
    ui_assets: Res<TerritoryUiAssets>,
    grid: Res<TerritoryGrid>,
    settings: Res<TerritorySettings>,
) {
    let _scope = profiler.scope(ScopeId::TerritorySpawnUnitsFromEvents);
    let profiling = profiler.is_enabled();
    let mut read_events = 0u64;
    for event in events.read() {
        if profiling {
            read_events += 1;
        }
        let (start_x, start_y) = event.team.start_corner();
        let corner_logic = grid.grid_to_logic(start_x, start_y);

        // 往中心方向偏移，确保在边界内
        let offset = Vec2::new(
            if start_x == 0 { 50.0 } else { -50.0 },
            if start_y == 0 { 50.0 } else { -50.0 },
        );
        let spawn_logic = corner_logic + offset;

        match event.action_type {
            ActionType::BigBall => {
                spawn_bigball(
                    &mut commands,
                    &render_assets,
                    &ui_assets.ui_font,
                    event.team,
                    event.value,
                    spawn_logic,
                );
            }
            ActionType::Shield => {
                // 护盾以HQ为中心
                spawn_shield(&mut commands, &render_assets, event.team, event.value, spawn_logic);
            }
            ActionType::MachineGun => {
                // 机关枪在HQ中心
                spawn_machine_gun(&mut commands, &render_assets, event.team, event.value, spawn_logic);
            }
            ActionType::CIWS => {
                // CIWS在HQ中心
                if settings.enable_ciws {
                    spawn_ciws(&mut commands, &render_assets, event.team, event.value, spawn_logic);
                }
            }
        }
    }
    profiler.add_counter(CounterId::TerritoryActionEventsRead, read_events);
}

fn spawn_bigball(
    commands: &mut Commands,
    render_assets: &TerritoryRenderAssets,
    ui_font: &Handle<Font>,
    team: TeamColor,
    size: u64,
    position: Vec2,
) {
    let mut rng = rand::thread_rng();
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let velocity = Vec2::new(angle.cos(), angle.sin()) * BIGBALL_SPEED;

    let mesh = render_assets.bigball_mesh.clone();
    let material = render_assets.team_material(team);

    let ball_entity = commands
        .spawn((
            BigBall { team, size },
            TerritoryUnit { team },
            LogicPosition(position),
            LastLogicPosition(position),
            KinematicVelocity(velocity),
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_translation(position.extend(1.0)),
        ))
        .insert(RenderLayers::layer(1))
        .id();

    commands.entity(ball_entity).with_children(|parent| {
        parent.spawn((
            BigBallValueText,
            RenderLayers::layer(1),
            Text2d::new(format_value(size)),
            TextFont {
                font: ui_font.clone(),
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_translation(Vec3::new(0.0, BIGBALL_RADIUS + 10.0, 1.0)),
        ));
    });
}

fn spawn_shield(
    commands: &mut Commands,
    render_assets: &TerritoryRenderAssets,
    team: TeamColor,
    durability: u64,
    position: Vec2,
) {
    let mesh = render_assets.shield_mesh.clone();
    let material = render_assets.shield_material(team);

    commands.spawn((
        Shield { team, durability, radius: SHIELD_RADIUS },
        TerritoryUnit { team },
        RenderLayers::layer(1),
        LogicPosition(position),
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.8)),
    ));
}

fn spawn_machine_gun(
    commands: &mut Commands,
    render_assets: &TerritoryRenderAssets,
    team: TeamColor,
    bullets: u64,
    position: Vec2,
) {
    let mesh = render_assets.machine_gun_mesh.clone();
    let material = render_assets.team_material(team);

    commands.spawn((
        MachineGun {
            team,
            bullets,
            fire_timer: Timer::from_seconds(0.2, TimerMode::Repeating),
            rotation: 0.0,
            rotation_speed: std::f32::consts::PI / 2.0,
        },
        TerritoryUnit { team },
        RenderLayers::layer(1),
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.9)),
    ));
}

fn spawn_ciws(
    commands: &mut Commands,
    render_assets: &TerritoryRenderAssets,
    team: TeamColor,
    bullets: u64,
    position: Vec2,
) {
    let mesh = render_assets.ciws_mesh.clone();
    let material = render_assets.team_material(team);

    commands.spawn((
        CIWS {
            team,
            bullets,
            fire_timer: Timer::from_seconds(0.3, TimerMode::Repeating),
        },
        TerritoryUnit { team },
        RenderLayers::layer(1),
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(0.9)),
    ));
}

/// 子弹合并比例：每 BULLET_MERGE_RATIO 颗逻辑子弹合并为 1 颗实体子弹
const BULLET_MERGE_RATIO: u64 = 100;

/// 机关枪旋转射击
pub fn machine_gun_rotate_fire(
    profiler: Res<Profiler>,
    mut commands: Commands,
    time: Res<Time>,
    mut machine_guns: Query<(&mut MachineGun, &Transform)>,
    render_assets: Res<TerritoryRenderAssets>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryMachineGunRotateFire);
    let delta = time.delta_secs();

    for (mut gun, gun_transform) in machine_guns.iter_mut() {
        gun.rotation += gun.rotation_speed * delta;

        // 计算这一帧应该发射多少颗逻辑子弹
        let fire_interval = gun.fire_timer.duration().as_secs_f32();
        let logical_bullets = (delta / fire_interval) as u64;
        let bullets_to_consume = logical_bullets.min(gun.bullets);

        if bullets_to_consume == 0 {
            continue;
        }

        gun.bullets -= bullets_to_consume;

        // 合并子弹：每 BULLET_MERGE_RATIO 颗合并为 1 颗实体
        let merged_count = (bullets_to_consume + BULLET_MERGE_RATIO - 1) / BULLET_MERGE_RATIO;
        let value_per_bullet = bullets_to_consume / merged_count;
        let mut remainder = bullets_to_consume % merged_count;

        for _ in 0..merged_count {
            let direction = Vec2::new(gun.rotation.cos(), gun.rotation.sin());
            // 分配余数到前几颗子弹
            let extra = if remainder > 0 { remainder -= 1; 1 } else { 0 };
            spawn_bullet(
                &mut commands,
                &render_assets,
                gun.team,
                value_per_bullet + extra,
                gun_transform.translation.truncate(),
                direction,
            );

            // 每颗实体子弹旋转一点
            gun.rotation += 0.00001 * BULLET_MERGE_RATIO as f32;
        }
    }
}

/// 近防炮瞄准最近敌人射击
pub fn ciws_target_fire(
    profiler: Res<Profiler>,
    mut commands: Commands,
    time: Res<Time>,
    mut ciws_query: Query<(&mut CIWS, &Transform)>,
    target_index: Res<TargetSpatialIndex>,
    render_assets: Res<TerritoryRenderAssets>,
    settings: Res<TerritorySettings>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryCiwsTargetFire);
    for (mut ciws, ciws_transform) in ciws_query.iter_mut() {
        ciws.fire_timer.tick(time.delta());

        if !ciws.fire_timer.just_finished() || ciws.bullets == 0 {
            continue;
        }

        let ciws_pos = ciws_transform.translation.truncate();
        if let Some(target_pos) = target_index.nearest_enemy_pos(ciws_pos, ciws.team, settings.ciws_distance_metric) {
            // 合并子弹：消耗最多 BULLET_MERGE_RATIO 颗，发射 1 颗高 value 子弹
            let bullets_to_consume = BULLET_MERGE_RATIO.min(ciws.bullets);
            ciws.bullets -= bullets_to_consume;
            let direction = (target_pos - ciws_pos).normalize();
            spawn_bullet(&mut commands, &render_assets, ciws.team, bullets_to_consume, ciws_pos, direction);
        }
    }
}

fn spawn_bullet(
    commands: &mut Commands,
    render_assets: &TerritoryRenderAssets,
    team: TeamColor,
    value: u64,
    position: Vec2,
    direction: Vec2,
) {
    let velocity = direction * BULLET_SPEED;
    let angle = direction.y.atan2(direction.x);

    let mesh = render_assets.bullet_mesh.clone();
    let material = render_assets.team_material(team);

    commands.spawn((
        Bullet { team, value },
        RenderLayers::layer(1),
        LogicPosition(position),
        LastLogicPosition(position),
        BulletPrevPosition(position),
        KinematicVelocity(velocity),
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(1.5)).with_rotation(Quat::from_rotation_z(angle)),
    ));
}

/// 积分子弹（FixedUpdate）：边界反射 + 最低速度保证 + 记录上一位置用于连续碰撞检测
pub fn bullet_integrate(
    profiler: Res<Profiler>,
    time: Res<Time>,
    mut bullets: Query<(&mut Transform, &mut KinematicVelocity, &mut BulletPrevPosition), With<Bullet>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBulletMove);
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let bullet_half = BULLET_BOUND_HALF;
    let min_x = -TERRITORY_LOGIC_WIDTH / 2.0 + bullet_half;
    let max_x = TERRITORY_LOGIC_WIDTH / 2.0 - bullet_half;
    let min_y = -TERRITORY_LOGIC_HEIGHT / 2.0 + bullet_half;
    let max_y = TERRITORY_LOGIC_HEIGHT / 2.0 - bullet_half;

    for (mut transform, mut velocity, mut prev) in bullets.iter_mut() {
        let mut pos = transform.translation.truncate();
        prev.0 = pos;
        pos += velocity.0 * dt;

        // X 轴
        if pos.x < min_x {
            pos.x = min_x;
            velocity.0.x = velocity.0.x.abs();
        } else if pos.x > max_x {
            pos.x = max_x;
            velocity.0.x = -velocity.0.x.abs();
        }

        // Y 轴
        if pos.y < min_y {
            pos.y = min_y;
            velocity.0.y = velocity.0.y.abs();
        } else if pos.y > max_y {
            pos.y = max_y;
            velocity.0.y = -velocity.0.y.abs();
        }

        let speed = velocity.0.length();
        if speed < BULLET_MIN_SPEED && speed > 0.0 {
            velocity.0 = velocity.0.normalize() * BULLET_MIN_SPEED;
        }

        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }
}

/// 积分大球（FixedUpdate）：边界反射
pub fn bigball_integrate(
    profiler: Res<Profiler>,
    time: Res<Time>,
    mut bigballs: Query<(&mut Transform, &mut KinematicVelocity), With<BigBall>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryContainUnits);
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let r = BIGBALL_RADIUS;
    let min_x = -TERRITORY_LOGIC_WIDTH / 2.0 + r;
    let max_x = TERRITORY_LOGIC_WIDTH / 2.0 - r;
    let min_y = -TERRITORY_LOGIC_HEIGHT / 2.0 + r;
    let max_y = TERRITORY_LOGIC_HEIGHT / 2.0 - r;

    for (mut transform, mut velocity) in bigballs.iter_mut() {
        let mut pos = transform.translation.truncate();
        pos += velocity.0 * dt;

        if pos.x < min_x {
            pos.x = min_x;
            velocity.0.x = velocity.0.x.abs();
        } else if pos.x > max_x {
            pos.x = max_x;
            velocity.0.x = -velocity.0.x.abs();
        }
        if pos.y < min_y {
            pos.y = min_y;
            velocity.0.y = velocity.0.y.abs();
        } else if pos.y > max_y {
            pos.y = max_y;
            velocity.0.y = -velocity.0.y.abs();
        }

        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }
}

/// 子弹击中地形 - 使用Bresenham追踪路径
pub fn bullet_hit_terrain(
    profiler: Res<Profiler>,
    mut commands: Commands,
    mut grid: ResMut<TerritoryGrid>,
    kernel: Res<BulletPaintKernel>,
    mut bullets: Query<(Entity, &mut Bullet, &Transform, &mut LastLogicPosition)>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBulletHitTerrain);
    let profiling = profiler.is_enabled();
    let mut cell_writes = 0u64;
    let mut path_points = 0u64;
    let mut kernel_points_est = 0u64;
    let kernel_points = kernel.points_est;
    for (entity, mut bullet, transform, mut last_pos) in bullets.iter_mut() {
        let current_logic = transform.translation.truncate();
        let _team_id = bullet.team.to_id();

        // 跳过第一帧（子弹还没移动）
        if last_pos.0 == current_logic {
            continue;
        }

        if let (Some((x0, y0)), Some((x1, y1))) = (grid.logic_to_grid(last_pos.0), grid.logic_to_grid(current_logic)) {
            'path: for (x, y) in bresenham_iter(x0 as i32, y0 as i32, x1 as i32, y1 as i32).skip(1) {
                if bullet.value == 0 {
                    break;
                }

                if x >= 0 && y >= 0 && x < grid.width as i32 && y < grid.height as i32 {
                    if profiling {
                        path_points += 1;
                        kernel_points_est += kernel_points;
                    }

                    for (dy, dx_max) in kernel.row_spans.iter().copied() {
                        if bullet.value == 0 {
                            break 'path;
                        }

                        let ny = y + dy;
                        if ny < 0 || ny >= grid.height as i32 {
                            continue;
                        }

                        let mut nx0 = x - dx_max;
                        let mut nx1 = x + dx_max;
                        if nx0 < 0 {
                            nx0 = 0;
                        }
                        if nx1 >= grid.width as i32 {
                            nx1 = grid.width as i32 - 1;
                        }
                        if nx0 > nx1 {
                            continue;
                        }

                        let painted = grid.paint_span_no_shield(
                            bullet.team,
                            ny as u32,
                            nx0 as u32,
                            nx1 as u32,
                            &mut bullet.value,
                        );
                        if profiling {
                            cell_writes += painted;
                        }
                    }
                }
            }

            if bullet.value == 0 {
                commands.entity(entity).despawn();
            }
        }

        last_pos.0 = current_logic;
    }
    profiler.add_counter(CounterId::TerritoryGridCellWrites, cell_writes);
    profiler.add_counter(CounterId::TerritoryBulletPathPoints, path_points);
    profiler.add_counter(CounterId::TerritoryBulletKernelPointsEst, kernel_points_est);
}

/// 子弹击中单位
fn segment_circle_t(a: Vec2, b: Vec2, c: Vec2, r: f32) -> Option<f32> {
    // Solve |(a + t*(b-a)) - c|^2 = r^2 for t in [0,1]
    let d = b - a;
    let f = a - c;
    let a0 = d.dot(d);
    if a0 <= 1e-8 {
        return None;
    }
    let b0 = 2.0 * f.dot(d);
    let c0 = f.dot(f) - r * r;
    let disc = b0 * b0 - 4.0 * a0 * c0;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t1 = (-b0 - s) / (2.0 * a0);
    let t2 = (-b0 + s) / (2.0 * a0);
    let mut best = None;
    if (0.0..=1.0).contains(&t1) {
        best = Some(t1);
    }
    if (0.0..=1.0).contains(&t2) {
        best = match best {
            None => Some(t2),
            Some(t) => Some(t.min(t2)),
        };
    }
    best
}

/// 子弹击中单位（FixedUpdate，自定义连续碰撞检测）
pub fn bullet_hit_units_manual(
    profiler: Res<Profiler>,
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet, &Transform, &BulletPrevPosition)>,
    mut bigballs: Query<(Entity, &mut BigBall, &Transform)>,
    mut shields: Query<(Entity, &mut Shield, &Transform)>,
    hqs: Query<(Entity, &HQ, &Transform)>,
    mut destroyed_events: MessageWriter<UnitDestroyedEvent>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBulletHitUnits);

    for (bullet_entity, bullet, transform, prev) in bullets.iter() {
        let a = prev.0;
        let b = transform.translation.truncate();
        let mut best: Option<(f32, Entity, HitKind)> = None;

        for (entity, hq, t) in hqs.iter() {
            if hq.team == bullet.team {
                continue;
            }
            let center = t.translation.truncate();
            if let Some(t_hit) = segment_circle_t(a, b, center, HQ_HALF_SIZE + BULLET_RADIUS) {
                if best.map_or(true, |(bt, _, _)| t_hit < bt) {
                    best = Some((t_hit, entity, HitKind::Hq));
                }
            }
        }

        for (entity, shield, t) in shields.iter() {
            if shield.team == bullet.team {
                continue;
            }
            let center = t.translation.truncate();
            if let Some(t_hit) = segment_circle_t(a, b, center, shield.radius + BULLET_RADIUS) {
                if best.map_or(true, |(bt, _, _)| t_hit < bt) {
                    best = Some((t_hit, entity, HitKind::Shield));
                }
            }
        }

        for (entity, ball, t) in bigballs.iter() {
            if ball.team == bullet.team {
                continue;
            }
            let center = t.translation.truncate();
            if let Some(t_hit) = segment_circle_t(a, b, center, BIGBALL_RADIUS + BULLET_RADIUS) {
                if best.map_or(true, |(bt, _, _)| t_hit < bt) {
                    best = Some((t_hit, entity, HitKind::BigBall));
                }
            }
        }

        let Some((_t_hit, target, kind)) = best else {
            continue;
        };

        match kind {
            HitKind::Hq => {
                // HQ 被击中即摧毁（无耐久），发送销毁事件
                // 胜利由 check_victory 系统判断（当只剩一队时）
                if let Ok((_, hq, _)) = hqs.get(target) {
                    destroyed_events.write(UnitDestroyedEvent { team: hq.team, entity: target });
                }
                commands.entity(target).despawn();
                commands.entity(bullet_entity).despawn();
            }
            HitKind::Shield => {
                if let Ok((_e, mut shield, _t)) = shields.get_mut(target) {
                    let damage = bullet.value.min(shield.durability);
                    shield.durability -= damage;
                }
                commands.entity(bullet_entity).despawn();
                if let Ok((_e, shield, _t)) = shields.get(target) {
                    if shield.durability == 0 {
                        commands.entity(target).despawn();
                        destroyed_events.write(UnitDestroyedEvent { team: shield.team, entity: target });
                    }
                }
            }
            HitKind::BigBall => {
                if let Ok((_e, mut ball, _t)) = bigballs.get_mut(target) {
                    let damage = bullet.value.min(ball.size);
                    ball.size -= damage;
                }
                commands.entity(bullet_entity).despawn();
                if let Ok((_e, ball, _t)) = bigballs.get(target) {
                    if ball.size == 0 {
                        commands.entity(target).despawn();
                        destroyed_events.write(UnitDestroyedEvent { team: ball.team, entity: target });
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HitKind {
    Hq,
    Shield,
    BigBall,
}

/// 子弹与子弹碰撞
/// 子弹与子弹碰撞（FixedUpdate，可选；当前实现用粗暴 O(n^2)，仅用于 debug 开关）
pub fn bullet_bullet_collision_manual(
    profiler: Res<Profiler>,
    mut commands: Commands,
    settings: Res<TerritorySettings>,
    mut bullets: Query<(Entity, &mut Bullet, &Transform)>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBulletBulletCollision);
    if !settings.enable_bullet_bullet_collision {
        return;
    }

    let mut list: Vec<(Entity, TeamColor, u64, Vec2)> = Vec::new();
    for (e, b, t) in bullets.iter() {
        list.push((e, b.team, b.value, t.translation.truncate()));
    }

    for i in 0..list.len() {
        for j in (i + 1)..list.len() {
            let (e1, team1, _, p1) = list[i];
            let (e2, team2, _, p2) = list[j];
            if team1 == team2 {
                continue;
            }
            if p1.distance_squared(p2) > (BULLET_RADIUS * 2.0) * (BULLET_RADIUS * 2.0) {
                continue;
            }
            let Ok([(_, mut b1, _), (_, mut b2, _)]) = bullets.get_many_mut([e1, e2]) else {
                continue;
            };
            if b1.team == b2.team {
                continue;
            }
            if b1.value > b2.value {
                b1.value -= b2.value;
                commands.entity(e2).despawn();
            } else if b2.value > b1.value {
                b2.value -= b1.value;
                commands.entity(e1).despawn();
            } else {
                commands.entity(e1).despawn();
                commands.entity(e2).despawn();
            }
        }
    }
}

/// 大球占领格子 + 同步逻辑坐标
pub fn bigball_occupy_territory(
    profiler: Res<Profiler>,
    mut grid: ResMut<TerritoryGrid>,
    kernel: Res<BigBallPaintKernel>,
    mut bigballs: Query<(&mut BigBall, &Transform, &mut LogicPosition, &mut LastLogicPosition)>,
    shields: Query<(&Shield, &LogicPosition), Without<BigBall>>,
    mut shield_cache: Local<Vec<ShieldInfo>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBigballOccupyTerritory);
    let profiling = profiler.is_enabled();
    let mut occupied_cells = 0u64;
    shield_cache.clear();
    shield_cache.extend(shields.iter().map(|(shield, logic_pos)| ShieldInfo {
        pos: logic_pos.0,
        radius_sq: shield.radius * shield.radius,
        team: shield.team,
    }));

    for (mut ball, transform, mut logic_pos, mut last_logic_pos) in bigballs.iter_mut() {
        // 更新逻辑坐标
        logic_pos.0 = transform.translation.truncate();
        let current_pos = logic_pos.0;

        if let (Some((x0, y0)), Some((x1, y1))) = (grid.logic_to_grid(last_logic_pos.0), grid.logic_to_grid(current_pos)) {
            for (cx, cy) in bresenham_iter(x0 as i32, y0 as i32, x1 as i32, y1 as i32) {
                // 对于路径上的每个点，占领以它为中心的圆形区域（查表 offset）
                for (dx, dy) in kernel.offsets.iter().copied() {
                    let x = cx + dx;
                    let y = cy + dy;

                    if x >= 0 && y >= 0 && x < grid.width as i32 && y < grid.height as i32 {
                        if grid.occupy(x as u32, y as u32, ball.team, &shield_cache) {
                            if ball.size > 0 {
                                ball.size -= 1;
                                if profiling {
                                    occupied_cells += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        last_logic_pos.0 = current_pos;
    }
    profiler.add_counter(CounterId::TerritoryGridCellsOccupied, occupied_cells);
}

#[derive(Clone)]
struct BresenhamIter {
    x: i32,
    y: i32,
    x1: i32,
    y1: i32,
    dx: i32,
    dy: i32,
    sx: i32,
    sy: i32,
    err: i32,
    done: bool,
}

impl Iterator for BresenhamIter {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let point = (self.x, self.y);
        if self.x == self.x1 && self.y == self.y1 {
            self.done = true;
            return Some(point);
        }

        let e2 = 2 * self.err;
        if e2 > -self.dy {
            self.err -= self.dy;
            self.x += self.sx;
        }
        if e2 < self.dx {
            self.err += self.dx;
            self.y += self.sy;
        }

        Some(point)
    }
}

fn bresenham_iter(x0: i32, y0: i32, x1: i32, y1: i32) -> BresenhamIter {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    BresenhamIter {
        x: x0,
        y: y0,
        x1,
        y1,
        dx,
        dy,
        sx,
        sy,
        err: dx - dy,
        done: false,
    }
}

/// 大球碰撞（动量守恒+损失数值）
pub fn bigball_collision(
    profiler: Res<Profiler>,
    mut bigballs: Query<(Entity, &mut BigBall, &mut Transform, &mut KinematicVelocity)>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBigballCollision);
    let mut ids = Vec::new();
    for (e, _, _, _) in bigballs.iter() {
        ids.push(e);
    }

    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let a = ids[i];
            let b = ids[j];
            let Ok([(ea, mut ball1, mut t1, mut v1), (eb, mut ball2, mut t2, mut v2)]) =
                bigballs.get_many_mut([a, b])
            else {
                continue;
            };
            let _ = (ea, eb);

            if ball1.team == ball2.team {
                continue;
            }

            let p1 = t1.translation.truncate();
            let p2 = t2.translation.truncate();
            let delta = p2 - p1;
            let dist_sq = delta.length_squared();
            let r = BIGBALL_RADIUS * 2.0;
            if dist_sq >= r * r || dist_sq <= 1e-8 {
                continue;
            }

            let dist = dist_sq.sqrt();
            let n = delta / dist;
            let penetration = r - dist;
            let corr = n * (penetration * 0.5);
            t1.translation.x -= corr.x;
            t1.translation.y -= corr.y;
            t2.translation.x += corr.x;
            t2.translation.y += corr.y;

            let m1 = (ball1.size.min(1000) as f32).max(1.0);
            let m2 = (ball2.size.min(1000) as f32).max(1.0);
            let rel = v1.0 - v2.0;
            let rel_n = rel.dot(n);
            if rel_n < 0.0 {
                let e = 0.9;
                let j_imp = (-(1.0 + e) * rel_n) / (1.0 / m1 + 1.0 / m2);
                let impulse = j_imp * n;
                v1.0 += impulse / m1;
                v2.0 -= impulse / m2;
            }

            let loss1 = ball2.size / 10;
            let loss2 = ball1.size / 10;
            ball1.size = ball1.size.saturating_sub(loss1);
            ball2.size = ball2.size.saturating_sub(loss2);
        }
    }
}

/// 大球撞击 HQ - 摧毁敌方 HQ
pub fn bigball_hit_hq(
    profiler: Res<Profiler>,
    mut commands: Commands,
    bigballs: Query<(&BigBall, &Transform)>,
    hqs: Query<(Entity, &HQ, &Transform)>,
    mut destroyed_events: MessageWriter<UnitDestroyedEvent>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryBigballCollision); // 复用同一 scope
    
    for (ball, ball_transform) in bigballs.iter() {
        let ball_pos = ball_transform.translation.truncate();
        
        for (hq_entity, hq, hq_transform) in hqs.iter() {
            // 跳过己方 HQ
            if hq.team == ball.team {
                continue;
            }
            
            let hq_pos = hq_transform.translation.truncate();
            let dist_sq = ball_pos.distance_squared(hq_pos);
            let collision_dist = BIGBALL_RADIUS + HQ_HALF_SIZE;
            
            if dist_sq <= collision_dist * collision_dist {
                // 大球撞击敌方 HQ，摧毁 HQ
                destroyed_events.write(UnitDestroyedEvent { team: hq.team, entity: hq_entity });
                commands.entity(hq_entity).despawn();
            }
        }
    }
}

/// 清理耗尽的单位
pub fn cleanup_depleted_units(
    profiler: Res<Profiler>,
    mut commands: Commands,
    bigballs: Query<(Entity, &BigBall)>,
    machine_guns: Query<(Entity, &MachineGun)>,
    ciws_query: Query<(Entity, &CIWS)>,
    mut destroyed_events: MessageWriter<UnitDestroyedEvent>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryCleanupDepletedUnits);
    for (entity, ball) in bigballs.iter() {
        if ball.size == 0 {
            commands.entity(entity).despawn();
            destroyed_events.write(UnitDestroyedEvent { team: ball.team, entity });
        }
    }

    for (entity, gun) in machine_guns.iter() {
        if gun.bullets == 0 {
            commands.entity(entity).despawn();
            destroyed_events.write(UnitDestroyedEvent { team: gun.team, entity });
        }
    }

    for (entity, ciws) in ciws_query.iter() {
        if ciws.bullets == 0 {
            commands.entity(entity).despawn();
            destroyed_events.write(UnitDestroyedEvent { team: ciws.team, entity });
        }
    }
}

/// 检测胜利 - 当只剩一支队伍有单位时，该队获胜
pub fn check_victory(
    profiler: Res<Profiler>,
    units: Query<&TerritoryUnit>,
    mut game_over: ResMut<GameOver>,
    mut victory_events: MessageWriter<VictoryEvent>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryCheckVictory);
    
    // 如果游戏已结束，不再检测
    if game_over.winner.is_some() {
        return;
    }
    
    let mut team_counts = [0u32; 4];

    for unit in units.iter() {
        team_counts[unit.team.index()] += 1;
    }

    let mut alive_count = 0usize;
    let mut last_alive: Option<TeamColor> = None;
    for (i, count) in team_counts.iter().enumerate() {
        if *count > 0 {
            alive_count += 1;
            last_alive = match i {
                0 => Some(TeamColor::Red),
                1 => Some(TeamColor::Blue),
                2 => Some(TeamColor::Green),
                3 => Some(TeamColor::Yellow),
                _ => None,
            };
            if alive_count > 1 {
                break;
            }
        }
    }

    if alive_count == 1 {
        if let Some(winner) = last_alive {
            game_over.winner = Some(winner);
            victory_events.write(VictoryEvent { winner });
        }
    }
}

/// 限制单位在战场范围内 + 边界弹性反弹
pub fn contain_units(
    profiler: Res<Profiler>,
    mut bigballs: Query<(&mut LogicPosition, &Transform), With<BigBall>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryContainUnits);
    for (mut logic_pos, transform) in bigballs.iter_mut() {
        logic_pos.0 = transform.translation.truncate();
    }
}

/// 让子弹朝向与其当前速度方向一致（避免物理改变速度后渲染方向滞后）
pub fn sync_bullet_rotation_to_velocity(
    profiler: Res<Profiler>,
    mut bullets: Query<(&KinematicVelocity, &mut Transform), With<Bullet>>,
) {
    let _scope = profiler.scope(ScopeId::TerritorySyncBulletRotationToVelocity);
    for (velocity, mut transform) in bullets.iter_mut() {
        let v = velocity.0;
        if v.length_squared() < 1e-6 {
            continue;
        }
        let angle = v.y.atan2(v.x);
        transform.rotation = Quat::from_rotation_z(angle);
    }
}

/// 更新大球数值文本（K/M/B）显示
pub fn update_bigball_value_text(
    profiler: Res<Profiler>,
    bigballs: Query<(Entity, &BigBall, Option<&Children>), Changed<BigBall>>,
    mut texts: Query<&mut Text2d, With<BigBallValueText>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryUpdateBigballValueText);
    for (_ball_entity, ball, children) in bigballs.iter() {
        let Some(children) = children else { continue; };
        let value = format_value(ball.size);
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 = value.clone();
            }
        }
    }
}

/// 处理胜利事件 - 暂停游戏并显示获胜信息
pub fn handle_victory_event(
    mut events: MessageReader<VictoryEvent>,
    mut time: ResMut<Time<Virtual>>,
) {
    for event in events.read() {
        info!("🎉 Victory! Team {:?} wins!", event.winner);
        // 暂停虚拟时间，停止游戏逻辑
        time.pause();
    }
}

/// 处理单位被消灭事件 - 记录日志
pub fn handle_unit_destroyed_event(
    mut events: MessageReader<UnitDestroyedEvent>,
) {
    for event in events.read() {
        info!("💥 Unit destroyed: Team {:?}, Entity {:?}", event.team, event.entity);
    }
}
