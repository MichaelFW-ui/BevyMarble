//! 不启动窗口，使用真实 ECS 战斗系统模拟中后期负载。

use std::time::{Duration, Instant};

use bevy::asset::{AssetApp, AssetPlugin};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::colors::TeamColor;
use crate::events::{UnitDestroyedEvent, VictoryEvent};
use crate::profiler::Profiler;

use super::components::*;
use super::grid::TerritoryGrid;
use super::plugin::{GameOver, TerritorySettings};
use super::render::{GridMaterial, setup_grid_render, update_grid_render};
use super::systems::*;

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    bullets: usize,
    bigballs: usize,
    shields: usize,
    machine_guns: usize,
    ciws: usize,
}

const MID: Scenario = Scenario {
    name: "mid",
    bullets: 1_200,
    bigballs: 120,
    shields: 24,
    machine_guns: 8,
    ciws: 12,
};

const LATE: Scenario = Scenario {
    name: "late",
    bullets: 4_000,
    bigballs: 400,
    shields: 80,
    machine_guns: 16,
    ciws: 48,
};

pub fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut scenario = "both".to_owned();
    let mut frames = 30usize;
    let mut warmup = 5usize;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| format!("缺少 {arg} 的值"))?;
        match arg.as_str() {
            "--scenario" if matches!(value.as_str(), "mid" | "late" | "both") => scenario = value,
            "--frames" => frames = value.parse().map_err(|_| "--frames 需要正整数")?,
            "--warmup" => warmup = value.parse().map_err(|_| "--warmup 需要非负整数")?,
            _ => return Err(format!("无效参数：{arg} {value}。用法：--scenario mid|late|both --frames N --warmup N")),
        }
    }
    if frames == 0 {
        return Err("--frames 必须大于 0".to_owned());
    }
    for config in [MID, LATE] {
        if scenario == "both" || scenario == config.name {
            run_scenario(config, frames, warmup);
        }
    }
    Ok(())
}

fn run_scenario(config: Scenario, frames: usize, warmup: usize) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<ColorMaterial>()
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_asset::<GridMaterial>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)))
        .insert_resource(TerritoryGrid::new(1024, 1024))
        .insert_resource(TerritorySettings::default())
        .init_resource::<GameOver>()
        .init_resource::<Profiler>()
        .init_resource::<BulletPaintKernel>()
        .init_resource::<TargetSpatialIndex>()
        .init_resource::<CollisionSpatialIndex>()
        .init_resource::<PendingDespawns>()
        .add_message::<VictoryEvent>()
        .add_message::<UnitDestroyedEvent>()
        .add_systems(Startup, (setup_territory_assets, setup_grid_render))
        .add_systems(Update, (
            clear_pending_despawns,
            machine_gun_rotate_fire,
            bigball_integrate,
            bullet_integrate,
            update_collision_spatial_index,
            update_target_spatial_index,
            ciws_target_fire,
            bullet_hit_terrain,
            bullet_hit_units_manual,
            bullet_bullet_collision_manual,
            bigball_collision,
            bigball_hit_hq,
            bigball_occupy_territory,
            cleanup_depleted_units,
            check_victory,
            update_grid_render,
        ).chain());

    let mut rng = StdRng::seed_from_u64(0xBEE5_2026);
    populate(app.world_mut(), config, &mut rng);

    for _ in 0..warmup {
        replenish_scene(app.world_mut(), config, &mut rng);
        app.update();
    }
    let profiler = app.world().resource::<Profiler>();
    profiler.take_scope_samples();
    profiler.take_counters();

    let mut frame_times = Vec::with_capacity(frames);
    let mut min_bullets = usize::MAX;
    let mut min_bigballs = usize::MAX;
    for _ in 0..frames {
        replenish_scene(app.world_mut(), config, &mut rng);
        let world = app.world_mut();
        min_bullets = min_bullets.min(world.query::<&Bullet>().iter(world).count());
        min_bigballs = min_bigballs.min(world.query::<&BigBall>().iter(world).count());
        let start = Instant::now();
        app.update();
        frame_times.push(start.elapsed().as_secs_f64() * 1_000.0);
    }

    frame_times.sort_by(f64::total_cmp);
    let avg = frame_times.iter().sum::<f64>() / frames as f64;
    let p95 = frame_times[((frames as f64 * 0.95).ceil() as usize).saturating_sub(1).min(frames - 1)];
    println!("\n场景 {}：{} 子弹 / {} 大球 / {} 护盾 / {} 机枪 / {} 近防炮；预热 {} 帧，测量 {} 帧", config.name, config.bullets, config.bigballs, config.shields, config.machine_guns, config.ciws, warmup, frames);
    println!("计时前最小实体数：{} 子弹 / {} 大球", min_bullets, min_bigballs);
    println!("ECS 帧耗时（不含场景补充和 GPU）：平均 {:.2} ms，P95 {:.2} ms，最大 {:.2} ms", avg, p95, frame_times[frames - 1]);

    let profiler = app.world().resource::<Profiler>();
    let mut samples = profiler.take_scope_samples();
    samples.retain(|(_, _, _, calls)| *calls > 0);
    samples.sort_by_key(|(_, sum, _, _)| std::cmp::Reverse(*sum));
    println!("{:44} {:>11} {:>11} {:>8}", "系统", "平均 ms/帧", "最大 ms/次", "调用");
    for (id, sum, max, calls) in samples {
        println!("{:44} {:>11.3} {:>11.3} {:>8}", id.name(), sum as f64 / frames as f64 / 1e6, max as f64 / 1e6, calls);
    }
    for (id, count) in profiler.take_counters() {
        if count != 0 {
            println!("计数 {:40} {:>12} /帧", id.name(), count / frames as u64);
        }
    }
}

fn populate(world: &mut World, config: Scenario, rng: &mut StdRng) {
    // 四块已有归属的土地，使刷地系统面对中后期的重涂场景。
    let mut grid = world.resource_mut::<TerritoryGrid>();
    for y in 0..grid.height {
        for x in 0..grid.width {
            let team = TeamColor::all()[((x / 512) + 2 * (y / 512)) as usize];
            grid.set(x, y, Some(team));
        }
    }
    drop(grid);

    for i in 0..config.bigballs {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        world.spawn((
            BigBall { team, size: 500_000 }, TerritoryUnit { team },
            LogicPosition(pos), LastLogicPosition(pos),
            KinematicVelocity(Vec2::from_angle(angle) * 100.0),
            Transform::from_translation(pos.extend(1.0)),
        ));
    }
    for i in 0..config.shields {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        world.spawn((
            Shield { team, durability: 1_000_000, radius: 50.0 }, TerritoryUnit { team },
            LogicPosition(pos), Transform::from_translation(pos.extend(0.8)),
        ));
    }
    for i in 0..config.machine_guns {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        world.spawn((
            MachineGun { team, bullets: 10_000_000, fire_timer: Timer::from_seconds(0.001, TimerMode::Repeating), rotation: 0.0, rotation_speed: std::f32::consts::TAU },
            TerritoryUnit { team }, Transform::from_translation(pos.extend(0.9)),
        ));
    }
    for i in 0..config.ciws {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        world.spawn((
            CIWS { team, bullets: 10_000_000, fire_timer: Timer::from_seconds(0.3, TimerMode::Repeating) },
            TerritoryUnit { team }, Transform::from_translation(pos.extend(0.9)),
        ));
    }
    for (i, team) in TeamColor::all().into_iter().enumerate() {
        let x = if i % 2 == 0 { -460.0 } else { 460.0 };
        let y = if i < 2 { -460.0 } else { 460.0 };
        world.spawn((HQ { team }, TerritoryUnit { team }, Transform::from_xyz(x, y, 2.0)));
    }
    replenish_scene(world, config, rng);
}

fn replenish_scene(world: &mut World, config: Scenario, rng: &mut StdRng) {
    replenish_bullets(world, config.bullets, rng);
    let bigballs = world.query::<&BigBall>().iter(world).count();
    for i in bigballs..config.bigballs {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        world.spawn((
            BigBall { team, size: 500_000 }, TerritoryUnit { team },
            LogicPosition(pos), LastLogicPosition(pos),
            KinematicVelocity(Vec2::from_angle(angle) * 100.0),
            Transform::from_translation(pos.extend(1.0)),
        ));
    }
    let shields = world.query::<&Shield>().iter(world).count();
    for i in shields..config.shields {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        world.spawn((
            Shield { team, durability: 1_000_000, radius: 50.0 }, TerritoryUnit { team },
            LogicPosition(pos), Transform::from_translation(pos.extend(0.8)),
        ));
    }
    let machine_guns = world.query::<&MachineGun>().iter(world).count();
    for i in machine_guns..config.machine_guns {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        world.spawn((
            MachineGun { team, bullets: 10_000_000, fire_timer: Timer::from_seconds(0.001, TimerMode::Repeating), rotation: 0.0, rotation_speed: std::f32::consts::TAU },
            TerritoryUnit { team }, Transform::from_translation(pos.extend(0.9)),
        ));
    }
    let ciws = world.query::<&CIWS>().iter(world).count();
    for i in ciws..config.ciws {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        world.spawn((
            CIWS { team, bullets: 10_000_000, fire_timer: Timer::from_seconds(0.3, TimerMode::Repeating) },
            TerritoryUnit { team }, Transform::from_translation(pos.extend(0.9)),
        ));
    }
}

fn replenish_bullets(world: &mut World, target: usize, rng: &mut StdRng) {
    let current = world.query::<&Bullet>().iter(world).count();
    for i in current..target {
        let team = TeamColor::all()[i % 4];
        let pos = random_pos(rng);
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        world.spawn((
            Bullet { team, value: 100 },
            LogicPosition(pos), LastLogicPosition(pos), BulletPrevPosition(pos),
            KinematicVelocity(Vec2::from_angle(angle) * 250.0),
            Transform::from_translation(pos.extend(1.5)),
        ));
    }
}

fn random_pos(rng: &mut StdRng) -> Vec2 {
    Vec2::new(rng.gen_range(-430.0..430.0), rng.gen_range(-430.0..430.0))
}
