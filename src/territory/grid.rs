use bevy::prelude::*;

use crate::colors::TeamColor;
use super::coords::{grid_to_logic, logic_to_grid};

/// Dirty tile tracking constants
pub const TILE_SIZE: u32 = 32;
pub const TILES_PER_ROW: u32 = 32; // 1024 / 32
pub const TOTAL_TILES: usize = 1024; // 32 * 32
const DIRTY_WORDS: usize = 16; // 1024 / 64

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
    row_words: usize,
    // 每队一份位图（按“行”对齐，方便按区间检查）
    team_row_bits: [Vec<u64>; 4],
    // 脏 tile 位图：1024 个 tile，用 16 个 u64 表示
    dirty_tiles: [u64; DIRTY_WORDS],
    dirty_count: u32,
}

impl TerritoryGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let total_cells = (width * height) as usize;
        let row_words = ((width as usize) + 63) / 64;
        let empty_bits = vec![0u64; (height as usize) * row_words];
        let mut grid = Self {
            cells: vec![0u8; total_cells],
            width,
            height,
            row_words,
            team_row_bits: [
                empty_bits.clone(),
                empty_bits.clone(),
                empty_bits.clone(),
                empty_bits,
            ],
            dirty_tiles: [u64::MAX; DIRTY_WORDS], // 初始全脏，确保首帧全量更新
            dirty_count: TOTAL_TILES as u32,
        };

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
                        grid.set_id(nx, ny, team.to_id());
                    }
                }
            }
        }

        grid
    }

    #[inline]
    fn row_bit_index(&self, x: u32, y: u32) -> (usize, u64) {
        let word = (x as usize) / 64;
        let bit = (x as usize) % 64;
        let idx = (y as usize) * self.row_words + word;
        (idx, 1u64 << bit)
    }

    #[inline]
    fn set_id(&mut self, x: u32, y: u32, new_id: u8) {
        let idx = (y * self.width + x) as usize;
        let old_id = self.cells[idx];
        if old_id == new_id {
            return;
        }

        let (word_idx, mask) = self.row_bit_index(x, y);
        if (1..=4).contains(&old_id) {
            let bits = &mut self.team_row_bits[(old_id - 1) as usize];
            bits[word_idx] &= !mask;
        }
        if (1..=4).contains(&new_id) {
            let bits = &mut self.team_row_bits[(new_id - 1) as usize];
            bits[word_idx] |= mask;
        }

        self.cells[idx] = new_id;
        
        // 标记所属 tile 为脏
        self.mark_tile_dirty(x, y);
    }
    
    /// 标记指定格子所属的 tile 为脏
    #[inline]
    fn mark_tile_dirty(&mut self, x: u32, y: u32) {
        let tile_x = x / TILE_SIZE;
        let tile_y = y / TILE_SIZE;
        let tile_idx = (tile_y * TILES_PER_ROW + tile_x) as usize;
        let word_idx = tile_idx / 64;
        let bit_mask = 1u64 << (tile_idx % 64);
        
        // 只有当这个 tile 之前未被标记时才增加计数
        if self.dirty_tiles[word_idx] & bit_mask == 0 {
            self.dirty_tiles[word_idx] |= bit_mask;
            self.dirty_count += 1;
        }
    }

    #[inline]
    pub fn cell_id(&self, x: u32, y: u32) -> u8 {
        self.cells[(y * self.width + x) as usize]
    }

    /// 证明式快速判断：这一行区间 [x0, x1] 是否全部属于 team_id（1..=4）。
    /// 只有在“全是己方”时才返回 true，因此不会漏掉任何空洞。
    pub fn row_all_team(&self, team_id: u8, y: u32, x0: u32, x1: u32) -> bool {
        if !(1..=4).contains(&team_id) {
            return false;
        }
        if y >= self.height || x0 > x1 || x1 >= self.width {
            return false;
        }
        let bits = &self.team_row_bits[(team_id - 1) as usize];

        let start_word = (x0 as usize) / 64;
        let end_word = (x1 as usize) / 64;
        let row_base = (y as usize) * self.row_words;

        let start_bit = (x0 as usize) % 64;
        let end_bit = (x1 as usize) % 64;

        if start_word == end_word {
            let mask = if end_bit == 63 {
                u64::MAX << start_bit
            } else {
                ((1u64 << (end_bit + 1)) - 1) & (u64::MAX << start_bit)
            };
            return (bits[row_base + start_word] & mask) == mask;
        }

        let start_mask = u64::MAX << start_bit;
        if (bits[row_base + start_word] & start_mask) != start_mask {
            return false;
        }

        for w in (start_word + 1)..end_word {
            if bits[row_base + w] != u64::MAX {
                return false;
            }
        }

        let end_mask = if end_bit == 63 { u64::MAX } else { (1u64 << (end_bit + 1)) - 1 };
        (bits[row_base + end_word] & end_mask) == end_mask
    }

    /// 在同一行 [x0, x1] 内，把所有“非己方”的格子改为 team，直到耗尽 budget。
    /// 返回本次实际染色（消耗 budget）的格子数。
    ///
    /// 这是确定性的、不会漏洞的优化：只枚举位图中不是 team 的 bit。
    pub fn paint_span_no_shield(&mut self, team: TeamColor, y: u32, x0: u32, x1: u32, budget: &mut u64) -> u64 {
        self.paint_span(team, y, x0, x1, budget, None)
    }

    /// 保护位图按行、每字 64 格排列；预算不足时仍从左到右占领。
    pub fn paint_span_protected(&mut self, team: TeamColor, y: u32, x0: u32, x1: u32, budget: &mut u64, protected: &[u64]) -> u64 {
        self.paint_span(team, y, x0, x1, budget, Some(protected))
    }

    fn paint_span(&mut self, team: TeamColor, y: u32, x0: u32, x1: u32, budget: &mut u64, protected: Option<&[u64]>) -> u64 {
        if *budget == 0 {
            return 0;
        }
        let team_id = team.to_id();
        if !(1..=4).contains(&team_id) {
            return 0;
        }
        if y >= self.height || x0 > x1 || x1 >= self.width {
            return 0;
        }
        if self.row_all_team(team_id, y, x0, x1) {
            return 0;
        }

        let start_word = (x0 as usize) / 64;
        let end_word = (x1 as usize) / 64;
        let row_base = (y as usize) * self.row_words;
        let start_bit = (x0 as usize) % 64;
        let end_bit = (x1 as usize) % 64;

        let mut painted = 0u64;
        for w in start_word..=end_word {
            if *budget == 0 {
                break;
            }
            let mut mask = u64::MAX;
            if w == start_word {
                mask &= u64::MAX << start_bit;
            }
            if w == end_word && end_bit != 63 {
                mask &= (1u64 << (end_bit + 1)) - 1;
            }
            if let Some(protected) = protected {
                mask &= !protected[w];
            }

            // NOTE: bits 是只读快照引用；set_id 会更新位图，但我们只用 holes 的逐 bit 枚举，
            // 且每个 bit 最多处理一次，不会漏涂。
            let word_idx = row_base + w;
            let team_bits = self.team_row_bits[(team_id - 1) as usize][word_idx];
            let mut holes = (!team_bits) & mask;
            let holes_count = holes.count_ones() as u64;
            if holes_count == 0 {
                continue;
            }

            // 预算足够时，整段一次改写：格子字节 + 四队位图。
            // 预算不足时保留从左到右逐格处理的原有语义。
            if *budget >= holes_count {
                let first = (w * 64).max(x0 as usize);
                let last = (w * 64 + 63).min(x1 as usize);
                let row_start = y as usize * self.width as usize;
                // 掩码中的连续可写区间批量填色，护盾覆盖格保持原值。
                let mut runs = mask;
                while runs != 0 {
                    let start = runs.trailing_zeros() as usize;
                    let len = (runs >> start).trailing_ones() as usize;
                    self.cells[row_start + w * 64 + start..row_start + w * 64 + start + len].fill(team_id);
                    if start + len == 64 { break; }
                    runs &= u64::MAX << (start + len);
                }
                for other in 0..4 {
                    if other == (team_id - 1) as usize {
                        self.team_row_bits[other][word_idx] |= mask;
                    } else {
                        self.team_row_bits[other][word_idx] &= !mask;
                    }
                }
                self.mark_tile_dirty(first as u32, y);
                self.mark_tile_dirty(last as u32, y);
                *budget -= holes_count;
                painted += holes_count;
                continue;
            }
            while holes != 0 && *budget != 0 {
                let bit = holes.trailing_zeros() as usize;
                holes &= holes - 1;
                let x = (w * 64 + bit) as u32;
                if x < self.width && self.cell_id(x, y) != team_id {
                    self.set_id(x, y, team_id);
                    *budget -= 1;
                    painted += 1;
                }
            }
        }

        painted
    }

    /// 获取指定位置的所属颜色
    pub fn get(&self, x: u32, y: u32) -> Option<TeamColor> {
        if x >= self.width || y >= self.height {
            return None;
        }
        TeamColor::from_id(self.cell_id(x, y))
    }

    /// 设置指定位置的所属颜色
    pub fn set(&mut self, x: u32, y: u32, team: Option<TeamColor>) {
        if x < self.width && y < self.height {
            self.set_id(x, y, team.map(|t| t.to_id()).unwrap_or(0u8));
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
            self.set_id(x, y, team.to_id());
            return true;
        }

        let logic_pos = grid_to_logic(x, y, self.width, self.height);

        // 检查是否有敌方护盾保护
        for shield in shields {
            if shield.team != team && shield.pos.distance_squared(logic_pos) <= shield.radius_sq {
                return false; // 被护盾保护，无法占领
            }
        }

        self.set_id(x, y, team.to_id());
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
    
    // ==================== 脏 Tile 追踪 API ====================
    
    /// 是否有脏 tile 需要更新
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty_count > 0
    }
    
    /// 获取脏 tile 数量
    #[inline]
    pub fn get_dirty_count(&self) -> u32 {
        self.dirty_count
    }

    /// 遍历所有脏 tile 的 (tile_x, tile_y) 坐标（零分配迭代器）
    pub fn iter_dirty_tiles(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.dirty_tiles.iter().enumerate().flat_map(|(word_idx, &word)| {
            let mut w = word;
            std::iter::from_fn(move || {
                if w == 0 {
                    return None;
                }
                let bit = w.trailing_zeros() as usize;
                w &= w - 1; // 清除最低位的 1
                let tile_idx = word_idx * 64 + bit;
                let tile_x = (tile_idx % TILES_PER_ROW as usize) as u32;
                let tile_y = (tile_idx / TILES_PER_ROW as usize) as u32;
                Some((tile_x, tile_y))
            })
        })
    }
    
    /// 清空所有脏标记
    pub fn clear_dirty(&mut self) {
        self.dirty_tiles = [0u64; DIRTY_WORDS];
        self.dirty_count = 0;
    }
    
    /// 获取指定 tile 对应的格子范围 (x_start, y_start, x_end_exclusive, y_end_exclusive)
    #[inline]
    pub fn tile_cell_range(tile_x: u32, tile_y: u32) -> (u32, u32, u32, u32) {
        let x_start = tile_x * TILE_SIZE;
        let y_start = tile_y * TILE_SIZE;
        let x_end = x_start + TILE_SIZE;
        let y_end = y_start + TILE_SIZE;
        (x_start, y_start, x_end, y_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_spans_match_cellwise_painting_and_budget_order() {
        let protection_patterns = [
            [0, 0, 0],
            [u64::MAX, u64::MAX, u64::MAX],
            [0xaaaaaaaaaaaaaaaa, 0x5555555555555555, 0x8000000000000001],
            [0xff00000000000000, 0xffff, 0],
        ];
        for protected in protection_patterns {
            for limit in [0, 1, 3, 25, 100, 200] {
                for team in TeamColor::all() {
                    let mut fast = TerritoryGrid::new(192, 64);
                    let mut reference = TerritoryGrid::new(192, 64);
                    for x in 0..192 {
                        let initial = TeamColor::all()[(x % 4) as usize];
                        fast.set(x, 20, Some(initial));
                        reference.set(x, 20, Some(initial));
                    }
                    fast.clear_dirty();
                    let mut budget = limit;
                    let painted = fast.paint_span_protected(team, 20, 28, 170, &mut budget, &protected);
                    let mut reference_budget = limit;
                    let mut reference_painted = 0;
                    for x in 28..=170 {
                        if reference_budget == 0 { break; }
                        if protected[x as usize / 64] & (1u64 << (x % 64)) == 0 && reference.get(x, 20) != Some(team) {
                            reference.set(x, 20, Some(team));
                            reference_budget -= 1;
                            reference_painted += 1;
                            let tile = (20 / TILE_SIZE * TILES_PER_ROW + x / TILE_SIZE) as usize;
                            assert_ne!(fast.dirty_tiles[tile / 64] & (1u64 << (tile % 64)), 0);
                        }
                    }
                    assert_eq!(painted, reference_painted);
                    assert_eq!(budget, reference_budget);
                    assert_eq!(fast.cells, reference.cells);
                    assert_eq!(fast.team_row_bits, reference.team_row_bits);
                }
            }
        }
    }

    #[test]
    fn bulk_span_matches_left_to_right_cell_updates() {
        for limit in [0, 1, 3, 25, 100] {
            let mut fast = TerritoryGrid::new(128, 64);
            let mut reference = TerritoryGrid::new(128, 64);
            for x in 0..128 {
                let team = TeamColor::all()[(x % 4) as usize];
                fast.set(x, 20, Some(team));
                reference.set(x, 20, Some(team));
            }

            let mut budget = limit;
            let painted = fast.paint_span_no_shield(TeamColor::Red, 20, 28, 90, &mut budget);
            let mut reference_budget = limit;
            let mut reference_painted = 0;
            for x in 28..=90 {
                if reference_budget == 0 {
                    break;
                }
                if reference.get(x, 20) != Some(TeamColor::Red) {
                    reference.set(x, 20, Some(TeamColor::Red));
                    reference_budget -= 1;
                    reference_painted += 1;
                }
            }
            assert_eq!(painted, reference_painted);
            assert_eq!(budget, reference_budget);
            assert_eq!(fast.cells(), reference.cells());
        }
    }
}
