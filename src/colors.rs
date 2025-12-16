use bevy::prelude::*;

/// 四种队伍颜色，从四个角落开始
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Default)]
pub enum TeamColor {
    #[default]
    Red,    // 左上角
    Blue,   // 右上角
    Green,  // 左下角
    Yellow, // 右下角
}

impl TeamColor {
    /// 获取队伍对应的显示颜色
    pub fn to_color(&self) -> Color {
        match self {
            TeamColor::Red => Color::srgb(0.9, 0.2, 0.2),
            TeamColor::Blue => Color::srgb(0.2, 0.4, 0.9),
            TeamColor::Green => Color::srgb(0.2, 0.8, 0.3),
            TeamColor::Yellow => Color::srgb(0.9, 0.85, 0.2),
        }
    }

    /// 获取所有队伍颜色
    pub fn all() -> [TeamColor; 4] {
        [
            TeamColor::Red,
            TeamColor::Blue,
            TeamColor::Green,
            TeamColor::Yellow,
        ]
    }

    /// 获取队伍在网格中的起始角落位置 (x, y)，基于1024x1024网格
    pub fn start_corner(&self) -> (u32, u32) {
        match self {
            TeamColor::Red => (0, 1023),          // 左上角
            TeamColor::Blue => (1023, 1023),      // 右上角
            TeamColor::Green => (0, 0),           // 左下角
            TeamColor::Yellow => (1023, 0),       // 右下角
        }
    }
}
