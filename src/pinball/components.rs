use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::utils::MAX_VALUE;
use crate::colors::TeamColor;

/// 弹珠小球组件
#[derive(Component, Debug, Clone)]
pub struct Marble {
    pub team: TeamColor,
    pub value: u64, // 初始数值来自 profile，上限为 32B
}

impl Marble {
    pub fn multiply(&mut self, multiplier: u64) {
        self.value = self.value.saturating_mul(multiplier).min(MAX_VALUE);
    }
}

#[derive(Component)]
pub struct PinballSceneEntity;

/// 加倍区域组件
#[derive(Component, Debug, Clone, Copy)]
pub struct MultiplierZone {
    pub multiplier: u64, // x2, x4, x8
    pub reset_position: bool,
}

/// 行动选择区域组件
#[derive(Component, Debug, Clone, Copy)]
pub struct ActionZone {
    pub action_type: ActionZoneType,
    pub value_scale: f32,
    pub reset_position: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BoostZone {
    pub velocity: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionZoneType {
    BigBall,
    Shield,
    MachineGun,
    #[serde(rename = "ciws")]
    CIWS,
}

/// 弹珠机边界墙标记
#[derive(Component, Debug, Clone, Copy)]
pub struct PinballWall;

/// 弹珠机钉子标记
#[derive(Component, Debug, Clone, Copy)]
pub struct PinballPeg;

/// 弹珠机起点标记（用于重置小球位置）
#[derive(Component, Debug, Clone, Copy)]
pub struct PinballSpawnPoint {
    pub team: TeamColor,
}

/// 小球数值文本标记
#[derive(Component, Debug, Clone, Copy)]
pub struct MarbleText {
    pub marble_entity: Entity,
}

/// 弹珠卡住检测（用于给一个“救援升力”避免长期静止）
#[derive(Component, Debug, Clone, Copy)]
pub struct StuckMarbleTracker {
    pub last_pos: Vec2,
    pub still_time: f32,
}
