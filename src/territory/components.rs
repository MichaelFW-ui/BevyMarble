use avian2d::prelude::*;
use bevy::prelude::*;

use crate::colors::TeamColor;

/// 碰撞层定义
#[derive(PhysicsLayer, Default, Clone, Copy, Debug)]
pub enum GameLayer {
    #[default]
    Default,
    RedTeam,
    BlueTeam,
    GreenTeam,
    YellowTeam,
}

pub const TEAM_LAYERS: [GameLayer; 4] = [
    GameLayer::RedTeam,
    GameLayer::BlueTeam,
    GameLayer::GreenTeam,
    GameLayer::YellowTeam,
];

impl TeamColor {
    /// 获取队伍对应的碰撞层
    pub fn to_layer(&self) -> GameLayer {
        match self {
            TeamColor::Red => GameLayer::RedTeam,
            TeamColor::Blue => GameLayer::BlueTeam,
            TeamColor::Green => GameLayer::GreenTeam,
            TeamColor::Yellow => GameLayer::YellowTeam,
        }
    }

    /// 获取敌方队伍的碰撞层（用于碰撞过滤）
    pub fn enemy_layers(&self) -> [GameLayer; 3] {
        match self {
            TeamColor::Red => [GameLayer::BlueTeam, GameLayer::GreenTeam, GameLayer::YellowTeam],
            TeamColor::Blue => [GameLayer::RedTeam, GameLayer::GreenTeam, GameLayer::YellowTeam],
            TeamColor::Green => [GameLayer::RedTeam, GameLayer::BlueTeam, GameLayer::YellowTeam],
            TeamColor::Yellow => [GameLayer::RedTeam, GameLayer::BlueTeam, GameLayer::GreenTeam],
        }
    }
}

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

/// 大球数值文本标记（作为大球的子实体）
#[derive(Component, Debug, Clone, Copy)]
pub struct BigBallValueText;
