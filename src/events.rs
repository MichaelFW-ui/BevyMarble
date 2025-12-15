use bevy::prelude::*;
use crate::colors::TeamColor;

/// 行动类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    BigBall,    // 大球 - 移动占领
    Shield,     // 护盾 - 固定防御
    MachineGun, // 机关枪 - 旋转射击
    CIWS,       // 近防炮 - 瞄准最近敌人
}

/// 弹珠机选择结果事件，触发领土战场的行动
#[derive(Message, Debug, Clone)]
pub struct ActionEvent {
    pub team: TeamColor,
    pub action_type: ActionType,
    pub value: u64, // 数值（体量/护盾量/子弹数）
}

/// 胜利事件
#[derive(Message, Debug, Clone)]
pub struct VictoryEvent {
    pub winner: TeamColor,
}

/// 单位被消灭事件
#[derive(Message, Debug, Clone)]
pub struct UnitDestroyedEvent {
    pub team: TeamColor,
    pub entity: Entity,
}
