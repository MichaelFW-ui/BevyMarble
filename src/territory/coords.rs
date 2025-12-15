use bevy::prelude::*;

/// 右侧战场逻辑坐标系统常量
pub const TERRITORY_LOGIC_WIDTH: f32 = 1024.0;
pub const TERRITORY_LOGIC_HEIGHT: f32 = 1024.0;

/// 渲染坐标系统常量
pub const TERRITORY_RENDER_WIDTH: f32 = 800.0;
pub const TERRITORY_RENDER_HEIGHT: f32 = 800.0;
pub const TERRITORY_RENDER_OFFSET_X: f32 = 200.0; // 右侧偏移（左侧弹珠机占400宽度，从-600到-200）

/// 逻辑坐标转渲染坐标
pub fn logic_to_render(logic_pos: Vec2) -> Vec2 {
    Vec2::new(
        (logic_pos.x / TERRITORY_LOGIC_WIDTH) * TERRITORY_RENDER_WIDTH - TERRITORY_RENDER_WIDTH / 2.0 + TERRITORY_RENDER_OFFSET_X,
        -((logic_pos.y / TERRITORY_LOGIC_HEIGHT) * TERRITORY_RENDER_HEIGHT - TERRITORY_RENDER_HEIGHT / 2.0),
    )
}

/// 渲染坐标转逻辑坐标
pub fn render_to_logic(render_pos: Vec2) -> Vec2 {
    let local_x = render_pos.x - TERRITORY_RENDER_OFFSET_X + TERRITORY_RENDER_WIDTH / 2.0;
    let local_y = -render_pos.y + TERRITORY_RENDER_HEIGHT / 2.0;

    Vec2::new(
        (local_x / TERRITORY_RENDER_WIDTH) * TERRITORY_LOGIC_WIDTH,
        (local_y / TERRITORY_RENDER_HEIGHT) * TERRITORY_LOGIC_HEIGHT,
    )
}

/// 格子索引转逻辑坐标（格子中心点）
pub fn grid_to_logic(grid_x: u32, grid_y: u32, grid_width: u32, grid_height: u32) -> Vec2 {
    Vec2::new(
        (grid_x as f32 + 0.5) * TERRITORY_LOGIC_WIDTH / grid_width as f32,
        (grid_y as f32 + 0.5) * TERRITORY_LOGIC_HEIGHT / grid_height as f32,
    )
}

/// 逻辑坐标转格子索引
pub fn logic_to_grid(logic_pos: Vec2, grid_width: u32, grid_height: u32) -> Option<(u32, u32)> {
    if logic_pos.x < 0.0 || logic_pos.y < 0.0 ||
       logic_pos.x >= TERRITORY_LOGIC_WIDTH || logic_pos.y >= TERRITORY_LOGIC_HEIGHT {
        return None;
    }

    let grid_x = ((logic_pos.x / TERRITORY_LOGIC_WIDTH) * grid_width as f32) as u32;
    let grid_y = ((logic_pos.y / TERRITORY_LOGIC_HEIGHT) * grid_height as f32) as u32;

    Some((grid_x.min(grid_width - 1), grid_y.min(grid_height - 1)))
}
