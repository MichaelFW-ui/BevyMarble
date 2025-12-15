use bevy::prelude::*;

use crate::colors::TeamColor;

/// 大球组件 - 移动占领格子
#[derive(Component, Debug, Clone)]
pub struct BigBall {
    pub team: TeamColor,
    pub size: u64, // 体量（剩余占领格数）
}

/// 护盾组件 - 固定位置防御
#[derive(Component, Debug, Clone)]
pub struct Shield {
    pub team: TeamColor,
    pub durability: u64, // 护盾量
    pub radius: f32,     // 保护范围
}

/// 机关枪组件 - 旋转射击
#[derive(Component, Debug, Clone)]
pub struct MachineGun {
    pub team: TeamColor,
    pub bullets: u64,      // 剩余子弹数
    pub fire_timer: Timer, // 射击冷却计时器
    pub rotation: f32,     // 当前旋转角度
    pub rotation_speed: f32, // 旋转速度（弧度/秒）
}

/// 近防炮组件 - 向最近的敌人大球或子弹开炮
#[derive(Component, Debug, Clone)]
pub struct CIWS {
    pub team: TeamColor,
    pub bullets: u64,      // 剩余子弹数
    pub fire_timer: Timer, // 射击冷却计时器
}

/// 总部（HQ）组件 - 被击中则失败
#[derive(Component, Debug, Clone)]
pub struct HQ {
    pub team: TeamColor,
}

/// 子弹组件
#[derive(Component, Debug, Clone)]
pub struct Bullet {
    pub team: TeamColor,
    pub value: u64, // 子弹数值，默认1
}

/// 领土单位标记（用于查询所有可以被消灭的单位）
#[derive(Component, Debug, Clone, Copy)]
pub struct TerritoryUnit {
    pub team: TeamColor,
}

/// 逻辑坐标（用于与格子系统交互）
#[derive(Component, Debug, Clone, Copy)]
pub struct LogicPosition(pub Vec2);

/// 上一帧的逻辑位置（用于追踪大球移动轨迹）
#[derive(Component, Debug, Clone, Copy)]
pub struct LastLogicPosition(pub Vec2);
