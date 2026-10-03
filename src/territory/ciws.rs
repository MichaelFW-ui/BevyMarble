use bevy::prelude::*;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

use super::coords::TERRITORY_LOGIC_WIDTH;

pub(super) const FIRE_INTERVAL: f32 = 0.3;

#[derive(Resource, Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CiwsConfig {
    /// 最大飞行路程，以战场边长为单位。
    pub max_range_battle_widths: f64,
    /// 最大路程处的数值保留比例，越过后回收。
    pub retained_value_at_max_range: f64,
    /// 归一化路程的指数，1 为线性衰减。
    pub falloff_exponent: f64,
    /// 单位命中数值占目标体量比例对应的速度变化。
    pub impulse_per_value: f32,
}

impl Default for CiwsConfig {
    fn default() -> Self {
        let path = Self::path();
        let json = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("读取 CIWS 配置 {} 失败：{error}", path.display()));
        Self::from_json(&json)
            .unwrap_or_else(|error| panic!("CIWS 配置 {} 无效：{error}", path.display()))
    }
}

impl CiwsConfig {
    pub fn path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/territory/ciws.json")
    }

    fn from_json(json: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if !config.max_range_battle_widths.is_finite() || config.max_range_battle_widths <= 0.0 {
            return Err("max_range_battle_widths 需要为有限正数".into());
        }
        if !config.retained_value_at_max_range.is_finite()
            || !(0.0..=1.0).contains(&config.retained_value_at_max_range)
        {
            return Err("retained_value_at_max_range 需要在 0 到 1 之间".into());
        }
        if !config.falloff_exponent.is_finite() || config.falloff_exponent <= 0.0 {
            return Err("falloff_exponent 需要为有限正数".into());
        }
        if !config.impulse_per_value.is_finite() || config.impulse_per_value < 0.0 {
            return Err("impulse_per_value 需要为有限非负数".into());
        }
        Ok(config)
    }

    pub fn max_range(&self) -> f64 {
        self.max_range_battle_widths * TERRITORY_LOGIC_WIDTH as f64
    }

    pub fn retained_fraction(&self, distance: f64) -> f64 {
        if distance > self.max_range() {
            return 0.0;
        }
        let progress = (distance / self.max_range()).clamp(0.0, 1.0);
        1.0 - (1.0 - self.retained_value_at_max_range) * progress.powf(self.falloff_exponent)
    }

    /// 按本段保留比例扣除剩余数值，携带舍入余量保证固定步拆分一致。
    pub fn decay_value(&self, value: u64, from: f64, to: f64, remainder: &mut f64) -> u64 {
        let before = self.retained_fraction(from);
        let after = self.retained_fraction(to);
        if before <= 0.0 || after <= 0.0 {
            return 0;
        }
        let ratio = (after / before).clamp(0.0, 1.0);
        let loss = value as f64 * (1.0 - ratio) + *remainder * ratio;
        let consumed = loss.round().clamp(0.0, value as f64) as u64;
        *remainder = loss - consumed as f64;
        value - consumed
    }
}

const REFERENCE_SIZE: f64 = 2_000_000.0;
const BASE_SHOT_VALUE: f64 = 25_000.0;
const SCALED_SHOT_VALUE: f64 = 100_000.0;
pub(super) const MAX_SHOT_VALUE: u64 = 200_000;
const MIN_FORWARD_RATIO: f32 = 0.8;

/// 按开火时目标的当前体量决定弹丸数值：2M 对应 125K，最大 200K。
pub(super) fn shot_value(size: u64) -> u64 {
    if size == 0 {
        return 0;
    }
    (BASE_SHOT_VALUE + SCALED_SHOT_VALUE * (size as f64 / REFERENCE_SIZE).sqrt())
        .min(MAX_SHOT_VALUE as f64)
        .round() as u64
}

/// 冲量沿弹丸方向，速度变化为 J / m；质量采用命中前体量。
pub(super) fn impact_velocity(
    config: &CiwsConfig,
    velocity: Vec2,
    direction: Vec2,
    size: u64,
    impact_value: u64,
) -> Vec2 {
    if size == 0 || impact_value == 0 {
        return velocity;
    }
    let Some(direction) = direction.try_normalize() else {
        return velocity;
    };
    let delta_speed = impact_value.min(size) as f32 / size as f32 * config.impulse_per_value;
    let mut result = velocity + direction * delta_speed;
    if let Some(forward) = velocity.try_normalize() {
        let min_forward_speed = velocity.length() * MIN_FORWARD_RATIO;
        let forward_speed = result.dot(forward);
        if forward_speed < min_forward_speed {
            result += forward * (min_forward_speed - forward_speed);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_falloff_retains_the_edge_value_then_reaches_zero() {
        let config = CiwsConfig::default();
        let edge = config.max_range();
        assert_eq!(config.retained_fraction(0.0), 1.0);
        assert!(
            (config.retained_fraction(edge) - config.retained_value_at_max_range).abs() < 1e-12
        );
        assert_eq!(config.retained_fraction(edge + 0.001), 0.0);
        let mut steeper = config.clone();
        steeper.falloff_exponent *= 2.0;
        assert!(steeper.retained_fraction(edge / 2.0) > config.retained_fraction(edge / 2.0));
    }

    #[test]
    fn distance_decay_is_independent_of_step_size_and_keeps_paint_consumption() {
        let config = CiwsConfig::default();
        let edge = config.max_range();
        for initial in [7, 125_000] {
            for steps in [1, 7, 60, 1000] {
                let mut value = initial;
                let mut remainder = 0.0;
                for step in 0..steps {
                    value = config.decay_value(
                        value,
                        edge * step as f64 / steps as f64,
                        edge * (step + 1) as f64 / steps as f64,
                        &mut remainder,
                    );
                }
                assert_eq!(
                    value,
                    (initial as f64 * config.retained_value_at_max_range).round() as u64,
                    "弹丸数值 {initial}，步数 {steps}"
                );
            }
        }
        let mut remainder = 0.0;
        let midpoint_value = config.decay_value(125_000, 0.0, edge / 2.0, &mut remainder);
        let painted_value = midpoint_value - 10_000;
        let remaining = config.decay_value(painted_value, edge / 2.0, edge, &mut remainder);
        let ratio = config.retained_fraction(edge) / config.retained_fraction(edge / 2.0);
        assert_eq!(remaining, (painted_value as f64 * ratio).round() as u64);
        assert!(remaining < (125_000.0 * config.retained_value_at_max_range).round() as u64);
    }

    #[test]
    fn range_configuration_rejects_invalid_values() {
        for json in [
            r#"{"max_range_battle_widths":0,"retained_value_at_max_range":0.2,"falloff_exponent":1,"impulse_per_value":60}"#,
            r#"{"max_range_battle_widths":0.7,"retained_value_at_max_range":1.1,"falloff_exponent":1,"impulse_per_value":60}"#,
            r#"{"max_range_battle_widths":0.7,"retained_value_at_max_range":0.2,"falloff_exponent":0,"impulse_per_value":60}"#,
            r#"{"max_range_battle_widths":0.7,"retained_value_at_max_range":0.2,"falloff_exponent":1,"impulse_per_value":-1}"#,
        ] {
            assert!(CiwsConfig::from_json(json).is_err());
        }
    }

    #[test]
    fn shot_value_tracks_current_target_mass_and_caps_for_large_balls() {
        let samples = [
            (50_000, 40_811),
            (500_000, 75_000),
            (1_000_000, 95_711),
            (2_000_000, 125_000),
            (4_000_000, 166_421),
            (8_000_000, 200_000),
        ];
        for (size, expected) in samples {
            assert_eq!(shot_value(size), expected);
        }
        assert_eq!(shot_value(u64::MAX), 200_000);
        assert_eq!(shot_value(0), 0);
    }

    #[test]
    fn repeated_head_on_impacts_keep_forward_motion() {
        let config = CiwsConfig::default();
        let mut size = 8_000_000;
        let mut velocity = Vec2::NEG_X * 100.0;
        for _ in 0..10 {
            let hit_damage = shot_value(size).min(size);
            let before = velocity;
            velocity = impact_velocity(&config, velocity, Vec2::X, size, hit_damage);
            assert!(velocity.x < 0.0, "迎面冲量使大球倒退：{velocity:?}");
            assert!(
                velocity.dot(before.normalize()) >= before.length() * MIN_FORWARD_RATIO - 0.001
            );
            assert_eq!(velocity.y, 0.0);
            size -= hit_damage;
        }
        assert!(size > 0);
        assert!(velocity.length() < 100.0);
    }

    #[test]
    fn oblique_impacts_deflect_with_smaller_acceleration_for_heavier_balls() {
        let config = CiwsConfig::default();
        let velocity = Vec2::new(-100.0, 20.0);
        let lighter = impact_velocity(&config, velocity, Vec2::X, 2_000_000, shot_value(2_000_000));
        let heavier = impact_velocity(&config, velocity, Vec2::X, 8_000_000, shot_value(8_000_000));
        assert!(lighter.x < 0.0 && heavier.x < 0.0);
        assert!(lighter.y.atan2(-lighter.x) > velocity.y.atan2(-velocity.x));
        assert!(heavier.y.atan2(-heavier.x) > velocity.y.atan2(-velocity.x));
        assert!((heavier - velocity).length() < (lighter - velocity).length());
    }
}
