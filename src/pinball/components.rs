use bevy::prelude::*;

use crate::colors::TeamColor;
use super::utils::MAX_VALUE;

/// 弹珠小球组件
#[derive(Component, Debug, Clone)]
pub struct Marble {
    pub team: TeamColor,
    pub value: u64, // 初始为2，经过加倍区后翻倍，最大32B
}

impl Marble {
    pub fn new(team: TeamColor) -> Self {
        Self { team, value: 1000 } // 从1K起步
    }

    pub fn reset(&mut self) {
        self.value = 1000;
    }

    pub fn multiply(&mut self, multiplier: u64) {
        self.value = self.value.saturating_mul(multiplier).min(MAX_VALUE);
    }
}

/// 加倍区域组件
#[derive(Component, Debug, Clone, Copy)]
pub struct MultiplierZone {
    pub multiplier: u64, // x2, x4, x8
}

/// 行动选择区域组件
#[derive(Component, Debug, Clone, Copy)]
pub struct ActionZone {
    pub action_type: ActionZoneType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionZoneType {
    BigBall,
    Shield,
    MachineGun,
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
