use bevy::prelude::*;

use crate::colors::TeamColor;
use super::coords::{grid_to_logic, logic_to_grid};

#[derive(Debug, Clone, Copy)]
pub struct ShieldInfo {
    pub pos: Vec2,
    pub radius_sq: f32,
    pub team: TeamColor,
}

/// 1024x1024 领土网格
#[derive(Resource, Debug)]
pub struct TerritoryGrid {
    // 0 表示空；1..=4 对应 TeamColor（见 TeamColor::to_id / from_id）
    cells: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl TerritoryGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let total_cells = (width * height) as usize;
        let mut cells = vec![0u8; total_cells];

        // 初始化四个角落（与 TeamColor::start_corner 一致）
        let corners = TeamColor::all().map(|team| {
            let (x, y) = team.start_corner();
            (team, x, y)
        });

        for (team, x, y) in corners {
            // 在角落占领一小块区域（5x5）
            for dy in 0..5 {
                for dx in 0..5 {
                    let nx = x.saturating_add(if x == 0 { dx } else { -(dx as i32) as u32 });
                    let ny = y.saturating_add(if y == 0 { dy } else { -(dy as i32) as u32 });
                    if nx < width && ny < height {
                        let index = (ny * width + nx) as usize;
                        cells[index] = team.to_id();
                    }
                }
            }
        }

        Self {
            cells,
            width,
            height,
        }
    }

    /// 获取指定位置的所属颜色
    pub fn get(&self, x: u32, y: u32) -> Option<TeamColor> {
        if x >= self.width || y >= self.height {
            return None;
        }
        TeamColor::from_id(self.cells[(y * self.width + x) as usize])
    }

    /// 设置指定位置的所属颜色
    pub fn set(&mut self, x: u32, y: u32, team: Option<TeamColor>) {
        if x < self.width && y < self.height {
            self.cells[(y * self.width + x) as usize] = team.map(|t| t.to_id()).unwrap_or(0u8);
        }
    }

    /// 占领指定位置（如果没有护盾保护），返回是否真的占领了新领土
    pub fn occupy(&mut self, x: u32, y: u32, team: TeamColor, shields: &[ShieldInfo]) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        // 如果已经是己方领土，不需要占领
        if self.get(x, y) == Some(team) {
            return false;
        }

        if shields.is_empty() {
            self.set(x, y, Some(team));
            return true;
        }

        let logic_pos = grid_to_logic(x, y, self.width, self.height);

        // 检查是否有敌方护盾保护
        for shield in shields {
            if shield.team != team && shield.pos.distance_squared(logic_pos) <= shield.radius_sq {
                return false; // 被护盾保护，无法占领
            }
        }

        self.set(x, y, Some(team));
        true // 成功占领了新领土
    }

    /// 逻辑坐标转网格坐标
    pub fn logic_to_grid(&self, logic_pos: Vec2) -> Option<(u32, u32)> {
        logic_to_grid(logic_pos, self.width, self.height)
    }

    /// 网格坐标转逻辑坐标
    pub fn grid_to_logic(&self, grid_x: u32, grid_y: u32) -> Vec2 {
        grid_to_logic(grid_x, grid_y, self.width, self.height)
    }

    /// 获取某个颜色占领的格子总数
    pub fn count_territory(&self, team: TeamColor) -> u32 {
        let id = team.to_id();
        self.cells
            .iter()
            .filter(|cell| **cell == id)
            .count() as u32
    }

    /// 获取原始网格数据（用于渲染）
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }
}
