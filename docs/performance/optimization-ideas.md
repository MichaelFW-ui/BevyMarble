# 早期优化方案

这份文档保留开发早期的优化思路，部分方案已实施或调整。当前实现与测量结果见[离线性能分析与优化记录](offline-profiler.md)。文中的源码行号来自记录时的版本，可能已发生变化。

## 网格渲染与变更跟踪

- 问题：TerritoryGrid 在 FixedUpdate 被多个系统以 ResMut 借用，导致 grid.is_changed() 几乎每帧为真，从而每帧全量 memcpy
  1024x1024 并触发材质重新 prepare。src/territory/render.rs:86 src/territory/systems.rs:706 src/territory/systems.rs:967
- 算法方案：用“分块脏标记 + 子纹理更新”的稀疏更新策略。把网格按固定 tile（如 32x32）分块，维护 dirty_tiles: BitSet 或
  Vec<bool>，set_id 只标记对应 tile。渲染更新时仅更新脏 tile 的子区域，不再全量复制；如果 Bevy 的 Image 不支持部分更新，就改
  用“多纹理 tile atlas”或 render 阶段 Queue::write_texture 做 subrect 写入。
- 实施要点：TerritoryGrid 内维护 dirty_tiles 和 dirty_count，set_id 标记；update_grid_render 读取 dirty tiles 并批量写；同时
  用显式 GridDirty 资源替代 grid.is_changed() 的隐式变更检测，避免 ResMut 借用导致假变更。

## 子弹涂地形（扫掠圆盘）

- 问题：bullet_hit_terrain 对每颗子弹做 Bresenham 路径点遍历，再对每个点执行圆形 kernel 行扫描，复杂度随子弹速度和数量线性叠
  加。src/territory/systems.rs:702
- 算法方案：把“逐点盖章”改为“胶囊体（线段 ⊕ 圆）”一次性栅格化。对子弹上一点到当前点的线段，计算其扫掠圆盘（capsule），用扫描
  线算法直接得到每行的 x 范围，然后一次调用 paint_span_no_shield。这把复杂度从 O(path_len * kernel) 降为 O(capsule_height)，
  并且更稳定。
- 实施要点：预先用整数坐标计算 capsule 的边界；每行求线段与水平线的交点范围，再合并两端圆帽的 x 范围；按行一次刷 span。对于
  极短位移，可退化为原先单点 kernel 以简化。

## 护盾判定 O(1) 化

- 问题：grid.occupy 在每个格子上都线性扫描所有护盾，嵌在大球路径遍历里，复杂度是 path * kernel * shields。src/territory/
  grid.rs:214 src/territory/systems.rs:964
- 算法方案：预计算“护盾覆盖网格”。维护 shield_mask[y][x]（每队 bitmask 或 per-team refcount），护盾生成/销毁时用圆形 kernel
  更新覆盖区域。占领时 O(1) 查掩码决定是否被敌方护盾保护。
- 实施要点：护盾是静态的，更新成本只在 spawn/despawn；如果未来护盾可移动，可增量更新或重建局部区域。

## 子弹打单位的连续碰撞

- 问题：每颗子弹遍历所有 HQ/护盾/大球做连续碰撞测试，没有空间剔除。src/territory/systems.rs:816
- 算法方案：构建“均匀网格 + 线段格子遍历”。每帧把 HQ/护盾/大球插入空间哈希（格子大小 ~ 最大半径）。对每颗子弹使用 Amanatides
  & Woo 线段穿格算法遍历格子，仅测试经过格子里的候选实体，保留最小 t 命中。
- 实施要点：HQ 可一次性静态插入；大球/护盾每帧更新索引；同一格子内测试数量远小于全量遍历。

## 大球碰撞（宽相）

- 问题：bigball_collision 两两配对 O(n^2)。src/territory/systems.rs:1075
- 算法方案：复用上面的均匀网格索引作为宽相，只在同格或邻格检查；半径固定时这一方法很稳定，复杂度近 O(n)。
- 实施要点：网格尺寸选择略大于 2*BIGBALL_RADIUS，避免漏检；相邻格检查 3x3 或 5x5。

## CIWS 最近目标

- 问题：nearest_enemy_pos 扩展环形扫描格子，若 CIWS 数量大且开启子弹为目标，会在 FixedUpdate 形成高成本。src/territory/
  systems.rs:198 src/territory/systems.rs:289
- 算法方案：针对 Manhattan 距离，用“多源 BFS 距离场”在目标栅格上一次性求最近敌人；每个 CIWS 查询 O(1)。若需 Euclidean，可用
  Felzenszwalb 2-pass 距离变换得到最近敌人位置（精确 L2²）。
- 实施要点：把目标位置投影到较粗网格（与 TargetSpatialIndex 对齐）；BFS/距离变换每帧一次，CIWS 直接读取结果。

## Pinball 文本同步

- 问题：更新/同步文本时，对每个 marble 扫所有 text（O(M*T)）。src/pinball/systems.rs:278 src/pinball/systems.rs:309
- 算法方案：维护 marble_entity -> text_entity 映射（或让文本成为 marble 子实体并通过 Children 直接定位），把查找变为 O(1)。

## 当时的实施顺序建议

1. 先做网格脏分块 + 子纹理更新（最大幅度降低每帧带宽与 render 阶段开销）。
2. 再做“护盾覆盖网格”与“胶囊体涂地形”（显著缩短 FixedUpdate 的热点）。
3. 最后上“空间哈希 + 线段穿格”用于子弹打单位与大球碰撞。
