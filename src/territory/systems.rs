use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use rand::Rng;

use crate::colors::TeamColor;
use crate::events::{ActionEvent, ActionType, UnitDestroyedEvent, VictoryEvent};
use crate::pinball::format_value;
use crate::territory::TerritorySettings;
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
    pub offsets: Vec<(i32, i32)>,
}

impl Default for BulletPaintKernel {
    fn default() -> Self {
        // 将子弹半径（游戏空间单位）换算为“格子半径”并预计算圆形覆盖 offset。
        // 这里不依赖任何渲染尺寸；只要 grid 的逻辑宽度不变，覆盖规则就稳定。
        let cells_per_unit = 1024.0 / TERRITORY_LOGIC_WIDTH;
        let radius = (BULLET_RADIUS * cells_per_unit).ceil().max(1.0) as i32;

        let mut offsets = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    offsets.push((dx, dy));
                }
            }
        }

        Self { offsets }
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

    fn nearest_enemy_pos(&self, origin: Vec2, team: TeamColor) -> Option<Vec2> {
        let (cx, cy) = self.cell_of(origin)?;

        let mut best: Option<(u64, f32, Vec2)> = None; // (entity_bits, dist2, pos)
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
                    let d2 = origin.distance_squared(entry.pos);
                    let bits = entry.entity.to_bits();
                    match best {
                        None => *best = Some((bits, d2, entry.pos)),
                        Some((best_bits, best_d2, _)) => {
                            if d2 + eps < *best_d2 || ((d2 - *best_d2).abs() <= eps && bits < *best_bits) {
                                *best = Some((bits, d2, entry.pos));
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

            if let Some((_, best_d2, _)) = best {
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
                    if boundary * boundary > best_d2 {
                        break;
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
    mut index: ResMut<TargetSpatialIndex>,
    targets: Query<(Entity, &TerritoryUnit, &Transform), Or<(With<BigBall>, With<Bullet>)>>,
) {
    index.clear();
    for (entity, unit, transform) in targets.iter() {
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
}

/// 响应行动事件，生成对应单位
pub fn spawn_units_from_events(
    mut commands: Commands,
    mut events: MessageReader<ActionEvent>,
    render_assets: Res<TerritoryRenderAssets>,
    ui_assets: Res<TerritoryUiAssets>,
    grid: Res<TerritoryGrid>,
) {
    for event in events.read() {
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
                spawn_ciws(&mut commands, &render_assets, event.team, event.value, spawn_logic);
            }
        }
    }
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
            RigidBody::Dynamic,
            Collider::circle(BIGBALL_RADIUS),
            // 只与敌方碰撞
            CollisionLayers::new([team.to_layer()], TEAM_LAYERS),
            LinearVelocity(velocity),
            Mass(size.min(1000) as f32),
            Restitution::new(0.9),
            GravityScale(0.0),
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_translation(position.extend(1.0)),
            CollisionEventsEnabled,
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
    mut commands: Commands,
    time: Res<Time>,
    mut machine_guns: Query<(&mut MachineGun, &Transform)>,
    render_assets: Res<TerritoryRenderAssets>,
) {
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
    mut commands: Commands,
    time: Res<Time>,
    mut ciws_query: Query<(&mut CIWS, &Transform)>,
    target_index: Res<TargetSpatialIndex>,
    render_assets: Res<TerritoryRenderAssets>,
) {
    for (mut ciws, ciws_transform) in ciws_query.iter_mut() {
        ciws.fire_timer.tick(time.delta());

        if !ciws.fire_timer.just_finished() || ciws.bullets == 0 {
            continue;
        }

        let ciws_pos = ciws_transform.translation.truncate();
        if let Some(target_pos) = target_index.nearest_enemy_pos(ciws_pos, ciws.team) {
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
        RigidBody::Dynamic,
        // 物理碰撞体用矩形近似即可（旋转会跟随 Transform）
        Collider::rectangle(BULLET_LENGTH, BULLET_WIDTH),
        // 只与敌方碰撞
        CollisionLayers::new([team.to_layer()], team.enemy_layers()),
        LinearVelocity(velocity),
        GravityScale(0.0),
        Mass(1.0),
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_translation(position.extend(1.5)).with_rotation(Quat::from_rotation_z(angle)),
    ));
}

/// 子弹边界反射 + 最低速度保证
pub fn bullet_move(
    mut bullets: Query<(&mut Transform, &mut LinearVelocity), With<Bullet>>,
) {
    let bullet_half = BULLET_BOUND_HALF;
    let min_x = -TERRITORY_LOGIC_WIDTH / 2.0 + bullet_half;
    let max_x = TERRITORY_LOGIC_WIDTH / 2.0 - bullet_half;
    let min_y = -TERRITORY_LOGIC_HEIGHT / 2.0 + bullet_half;
    let max_y = TERRITORY_LOGIC_HEIGHT / 2.0 - bullet_half;

    for (mut transform, mut velocity) in bullets.iter_mut() {
        let mut pos = transform.translation.truncate();

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

        // 保证最低速度
        let speed = velocity.0.length();
        if speed < BULLET_MIN_SPEED && speed > 0.0 {
            velocity.0 = velocity.0.normalize() * BULLET_MIN_SPEED;
        }

        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }
}

/// 子弹击中地形 - 使用Bresenham追踪路径
pub fn bullet_hit_terrain(
    mut commands: Commands,
    mut grid: ResMut<TerritoryGrid>,
    kernel: Res<BulletPaintKernel>,
    mut bullets: Query<(Entity, &mut Bullet, &Transform, &mut LastLogicPosition)>,
) {
    for (entity, mut bullet, transform, mut last_pos) in bullets.iter_mut() {
        let current_logic = transform.translation.truncate();

        // 跳过第一帧（子弹还没移动）
        if last_pos.0 == current_logic {
            continue;
        }

        if let (Some((x0, y0)), Some((x1, y1))) = (grid.logic_to_grid(last_pos.0), grid.logic_to_grid(current_logic)) {
            // 跳过起点，只染路径上的新格子（避免分配 Vec）
            for (x, y) in bresenham_iter(x0 as i32, y0 as i32, x1 as i32, y1 as i32).skip(1) {
                if bullet.value == 0 {
                    break;
                }

                if x >= 0 && y >= 0 && x < grid.width as i32 && y < grid.height as i32 {
                    // 以路径点为中心，染一个圆形区域（查表 offset，避免内层双循环）
                    for (dx, dy) in kernel.offsets.iter().copied() {
                        if bullet.value == 0 {
                            break;
                        }

                        let nx = x + dx;
                        let ny = y + dy;

                        if nx < 0 || ny < 0 || nx >= grid.width as i32 || ny >= grid.height as i32 {
                            continue;
                        }

                        let cell_team = grid.get(nx as u32, ny as u32);
                        // 只有空白或敌方领土才染色并消耗
                        if cell_team != Some(bullet.team) {
                            grid.set(nx as u32, ny as u32, Some(bullet.team));
                            bullet.value = bullet.value.saturating_sub(1);
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
}

/// 子弹击中单位
pub fn bullet_hit_units(
    mut commands: Commands,
    mut collision_events: MessageReader<CollisionStart>,
    bullets: Query<(&Bullet, Entity)>,
    mut bigballs: Query<(&mut BigBall, Entity)>,
    mut shields: Query<(&mut Shield, Entity)>,
    hqs: Query<(&HQ, Entity)>,
    mut destroyed_events: MessageWriter<UnitDestroyedEvent>,
    mut victory_events: MessageWriter<VictoryEvent>,
) {
    for event in collision_events.read() {
        let (bullet_entity, target_entity) = if bullets.contains(event.collider1) {
            (event.collider1, event.collider2)
        } else if bullets.contains(event.collider2) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };

        if let Ok((bullet, _)) = bullets.get(bullet_entity) {
            // 检查HQ
            if let Ok((hq, _)) = hqs.get(target_entity) {
                if hq.team != bullet.team {
                    commands.entity(bullet_entity).despawn();
                    victory_events.write(VictoryEvent { winner: bullet.team });
                    continue;
                }
            }

            // 检查大球
            if let Ok((mut ball, ball_entity)) = bigballs.get_mut(target_entity) {
                if ball.team != bullet.team {
                    let damage = bullet.value.min(ball.size);
                    ball.size -= damage;
                    commands.entity(bullet_entity).despawn();

                    if ball.size == 0 {
                        commands.entity(ball_entity).despawn();
                        destroyed_events.write(UnitDestroyedEvent { team: ball.team, entity: ball_entity });
                    }
                    continue;
                }
            }

            // 检查护盾
            if let Ok((mut shield, shield_entity)) = shields.get_mut(target_entity) {
                if shield.team != bullet.team {
                    let damage = bullet.value.min(shield.durability);
                    shield.durability -= damage;
                    commands.entity(bullet_entity).despawn();

                    if shield.durability == 0 {
                        commands.entity(shield_entity).despawn();
                        destroyed_events.write(UnitDestroyedEvent { team: shield.team, entity: shield_entity });
                    }
                }
            }
        }
    }
}

/// 子弹与子弹碰撞
pub fn bullet_bullet_collision(
    mut commands: Commands,
    settings: Res<TerritorySettings>,
    mut collision_events: MessageReader<CollisionStart>,
    mut bullets: Query<(&mut Bullet, Entity)>,
) {
    if !settings.enable_bullet_bullet_collision {
        return;
    }

    for event in collision_events.read() {
        // 使用 get_many_mut 避免同时借用冲突
        if let Ok([(mut bullet1, entity1), (mut bullet2, entity2)]) =
            bullets.get_many_mut([event.collider1, event.collider2]) {

            if bullet1.team != bullet2.team {
                if bullet1.value > bullet2.value {
                    bullet1.value -= bullet2.value;
                    commands.entity(entity2).despawn();
                } else if bullet2.value > bullet1.value {
                    bullet2.value -= bullet1.value;
                    commands.entity(entity1).despawn();
                } else {
                    commands.entity(entity1).despawn();
                    commands.entity(entity2).despawn();
                }
            }
        }
    }
}

/// 大球占领格子 + 同步逻辑坐标
pub fn bigball_occupy_territory(
    mut grid: ResMut<TerritoryGrid>,
    kernel: Res<BigBallPaintKernel>,
    mut bigballs: Query<(&mut BigBall, &Transform, &mut LogicPosition, &mut LastLogicPosition)>,
    shields: Query<(&Shield, &LogicPosition), Without<BigBall>>,
    mut shield_cache: Local<Vec<ShieldInfo>>,
) {
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
                            }
                        }
                    }
                }
            }
        }

        last_logic_pos.0 = current_pos;
    }
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
    mut collision_events: MessageReader<CollisionStart>,
    mut bigballs: Query<(&mut BigBall, &mut LinearVelocity, &Mass)>,
) {
    for event in collision_events.read() {
        // 使用 get_many_mut 避免同时借用冲突
        if let Ok([(mut ball1, mut vel1, mass1), (mut ball2, mut vel2, mass2)]) =
            bigballs.get_many_mut([event.collider1, event.collider2]) {

            if ball1.team != ball2.team {
                // 简单的弹性碰撞（动量守恒）
                let v1 = vel1.0;
                let v2 = vel2.0;
                let m1 = mass1.0;
                let m2 = mass2.0;

                vel1.0 = ((m1 - m2) * v1 + 2.0 * m2 * v2) / (m1 + m2);
                vel2.0 = ((m2 - m1) * v2 + 2.0 * m1 * v1) / (m1 + m2);

                // 碰撞损失数值
                ball1.size = ball1.size.saturating_sub(ball2.size / 10);
                ball2.size = ball2.size.saturating_sub(ball1.size / 10);
            }
        }
    }
}

/// 清理耗尽的单位
pub fn cleanup_depleted_units(
    mut commands: Commands,
    bigballs: Query<(Entity, &BigBall)>,
    machine_guns: Query<(Entity, &MachineGun)>,
    ciws_query: Query<(Entity, &CIWS)>,
    mut destroyed_events: MessageWriter<UnitDestroyedEvent>,
) {
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

/// 检测胜利
pub fn check_victory(
    units: Query<&TerritoryUnit>,
    mut victory_events: MessageWriter<VictoryEvent>,
) {
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
            victory_events.write(VictoryEvent { winner });
        }
    }
}

/// 限制单位在战场范围内 + 边界弹性反弹
pub fn contain_units(
    mut bigballs: Query<(&mut Transform, &mut LogicPosition, &mut LinearVelocity), With<BigBall>>,
) {
    let r = BIGBALL_RADIUS;
    let min_x = -TERRITORY_LOGIC_WIDTH / 2.0 + r;
    let max_x = TERRITORY_LOGIC_WIDTH / 2.0 - r;
    let min_y = -TERRITORY_LOGIC_HEIGHT / 2.0 + r;
    let max_y = TERRITORY_LOGIC_HEIGHT / 2.0 - r;

    for (mut transform, mut logic_pos, mut velocity) in bigballs.iter_mut() {
        logic_pos.0 = transform.translation.truncate();

        // X 轴
        if logic_pos.0.x < min_x {
            logic_pos.0.x = min_x;
            velocity.0.x = velocity.0.x.abs();
        } else if logic_pos.0.x > max_x {
            logic_pos.0.x = max_x;
            velocity.0.x = -velocity.0.x.abs();
        }

        // Y 轴
        if logic_pos.0.y < min_y {
            logic_pos.0.y = min_y;
            velocity.0.y = velocity.0.y.abs();
        } else if logic_pos.0.y > max_y {
            logic_pos.0.y = max_y;
            velocity.0.y = -velocity.0.y.abs();
        }

        transform.translation.x = logic_pos.0.x;
        transform.translation.y = logic_pos.0.y;
    }
}

/// 让子弹朝向与其当前速度方向一致（避免物理改变速度后渲染方向滞后）
pub fn sync_bullet_rotation_to_velocity(
    mut bullets: Query<(&LinearVelocity, &mut Transform), With<Bullet>>,
) {
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
    bigballs: Query<(Entity, &BigBall, Option<&Children>), Changed<BigBall>>,
    mut texts: Query<&mut Text2d, With<BigBallValueText>>,
) {
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
