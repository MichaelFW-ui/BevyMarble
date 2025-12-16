use bevy::prelude::*;

/// 战场“游戏空间”（world/logical space）尺寸。
///
/// 这里的坐标用于物理、碰撞、AI、占领等一切游戏逻辑，**不依赖渲染像素尺寸**。
/// 渲染（相机缩放、视口布局等）应在渲染层处理。
pub const TERRITORY_LOGIC_WIDTH: f32 = 1024.0;
pub const TERRITORY_LOGIC_HEIGHT: f32 = 1024.0;

#[inline]
fn half_extents() -> Vec2 {
    Vec2::new(TERRITORY_LOGIC_WIDTH / 2.0, TERRITORY_LOGIC_HEIGHT / 2.0)
}

/// 格子索引转逻辑坐标（格子中心点）
pub fn grid_to_logic(grid_x: u32, grid_y: u32, grid_width: u32, grid_height: u32) -> Vec2 {
    let half = half_extents();
    Vec2::new(
        (grid_x as f32 + 0.5) * TERRITORY_LOGIC_WIDTH / grid_width as f32 - half.x,
        (grid_y as f32 + 0.5) * TERRITORY_LOGIC_HEIGHT / grid_height as f32 - half.y,
    )
}

/// 逻辑坐标转格子索引
pub fn logic_to_grid(logic_pos: Vec2, grid_width: u32, grid_height: u32) -> Option<(u32, u32)> {
    let half = half_extents();
    let local = logic_pos + half;

    if local.x < 0.0 || local.y < 0.0 || local.x >= TERRITORY_LOGIC_WIDTH || local.y >= TERRITORY_LOGIC_HEIGHT {
        return None;
    }

    let grid_x = ((local.x / TERRITORY_LOGIC_WIDTH) * grid_width as f32) as u32;
    let grid_y = ((local.y / TERRITORY_LOGIC_HEIGHT) * grid_height as f32) as u32;

    Some((grid_x.min(grid_width - 1), grid_y.min(grid_height - 1)))
}
