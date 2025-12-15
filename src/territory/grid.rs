use bevy::prelude::*;

use crate::colors::TeamColor;
use super::coords::{grid_to_logic, logic_to_grid};

/// 1024x1024 领土网格
#[derive(Resource, Debug)]
pub struct TerritoryGrid {
    cells: Vec<Option<TeamColor>>,
    pub width: u32,
    pub height: u32,
}

impl TerritoryGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let total_cells = (width * height) as usize;
        let mut cells = vec![None; total_cells];

        // 初始化四个角落
        let corners = [
            (TeamColor::Red, 0, 0),                     // 左上
            (TeamColor::Blue, width - 1, 0),            // 右上
            (TeamColor::Green, 0, height - 1),          // 左下
            (TeamColor::Yellow, width - 1, height - 1), // 右下
        ];

        for (team, x, y) in corners {
            // 在角落占领一小块区域（5x5）
            for dy in 0..5 {
                for dx in 0..5 {
                    let nx = x.saturating_add(if x == 0 { dx } else { -(dx as i32) as u32 });
                    let ny = y.saturating_add(if y == 0 { dy } else { -(dy as i32) as u32 });
                    if nx < width && ny < height {
                        let index = (ny * width + nx) as usize;
                        cells[index] = Some(team);
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
        self.cells[(y * self.width + x) as usize]
    }

    /// 设置指定位置的所属颜色
    pub fn set(&mut self, x: u32, y: u32, team: Option<TeamColor>) {
        if x < self.width && y < self.height {
            self.cells[(y * self.width + x) as usize] = team;
        }
    }

    /// 占领指定位置（如果没有护盾保护），返回是否真的占领了新领土
    pub fn occupy(&mut self, x: u32, y: u32, team: TeamColor, shields: &[(Vec2, f32, TeamColor)]) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }

        // 如果已经是己方领土，不需要占领
        if self.get(x, y) == Some(team) {
            return false;
        }

        let logic_pos = grid_to_logic(x, y, self.width, self.height);

        // 检查是否有敌方护盾保护
        for (shield_pos, radius, shield_team) in shields {
            if *shield_team != team && shield_pos.distance(logic_pos) <= *radius {
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
        self.cells
            .iter()
            .filter(|cell| **cell == Some(team))
            .count() as u32
    }

    /// 获取原始网格数据（用于渲染）
    pub fn cells(&self) -> &[Option<TeamColor>] {
        &self.cells
    }
}
