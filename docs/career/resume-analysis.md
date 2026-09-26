# BevyMarble 项目分析

## 一、项目一句话概述

> 基于 **Bevy 0.17 ECS + Avian2D 物理** 的 2D 双玩法游戏原型：左侧弹珠机（物理驱动随机器）联动右侧 1024×1024 领土占领战场（四队实时对抗），两模块通过消息机制解耦通信。

---

## 二、简历上的项目叙述

**项目名**：BevyMarble — 基于 ECS 架构的 2D 双玩法游戏引擎原型

**技术栈**：Rust (Edition 2024) · Bevy 0.17 ECS · Avian2D Physics · WGSL Shader · 自定义空间索引 · 位图加速

**核心工作**：
- 使用 **Bevy ECS 的 Plugin 体系** 将弹珠机与领土战场拆分为两个独立插件，通过 `Message` 机制实现跨模块解耦通信
- 实现 **双相机分屏渲染**（Viewport + RenderLayers），游戏逻辑坐标与渲染坐标**完全隔离**，窗口尺寸变化不影响游戏规则
- 设计 **1024×1024 位图领土网格**：4 支队伍的按行位图 (`Vec<u64>`) 支持 O(1) 区间全属判定；`trailing_zeros()` + Brian Kernighan 算法实现高效空洞填充染色
- 实现 **脏 Tile 增量 GPU 更新**：32×32 分块 + 64-bit 位集标记，超过 25% 阈值自动切换全量更新；配合自定义 WGSL Shader 和 `R8Unorm` 纹理格式降低 GPU 带宽
- 构建两套 **均匀网格空间索引**：`CollisionSpatialIndex`（碰撞宽相）与 `TargetSpatialIndex`（CIWS 最近目标查询），使用活跃桶追踪避免 O(n) 清理
- 实现 **子弹连续碰撞检测**（线段 AABB + 空间索引过滤）、**预计算圆形覆盖核**（`BulletPaintKernel` / `BigBallPaintKernel` 避免运行时三角运算）、**弹珠卡死检测与自动恢复**
- 自研 **无锁性能分析器**：`AtomicU64` + RAII `ScopeGuard` + `Mutex<Option<Instant>>` 实现 23 个 scope 计时和 8 个 counter 计数，支持运行时开关和 HUD 叠加显示

---

## 三、技术亮点清单

| 层级 | 技术点 | 具体实现 |
|------|--------|----------|
| **架构** | ECS + Plugin 组合 | 两个独立 `Plugin` 通过 Bevy `Message`（`ActionEvent`/`VictoryEvent`）解耦，Pinball 不依赖 Territory |
| **架构** | 坐标隔离 | 游戏逻辑空间（`PINBALL_WIDTH/HEIGHT`、`TERRITORY_LOGIC_WIDTH=1024`）与渲染空间完全分离，通过 `Camera.viewport` + `ScalingMode::FixedHorizontal` 映射 |
| **数据结构** | 位图领土网格 | `TerritoryGrid`：`cells: Vec<u8>` + 4 队 `team_row_bits: [Vec<u64>; 4]`，按行对齐支持 O(row_words) 跨行全属检测 |
| **算法** | Brian Kernighan 位遍历 | `paint_span_no_shield` 中 `holes &= holes - 1` + `trailing_zeros()` 高效枚举非己方格 |
| **算法** | 均匀网格空间索引 | `CollisionSpatialIndex`（cell_size=60）+ `TargetSpatialIndex`（cell_size=50），`active_buckets: Vec<usize>` 只清理非空桶 |
| **算法** | 线段 AABB 碰撞查询 | `query_segment()` 对子弹起止点 expanded AABB 做格子遍历，为连续碰撞检测提供候选集 |
| **渲染** | 脏 Tile 增量上传 | 1024 网格 → 32×32 Tile（共 1024 块），`dirty_tiles: [u64; 16]` 位集 + `dirty_count`，阈值为 25% |
| **渲染** | 自定义 WGSL Shader | `GridMaterial` 派生 `AsBindGroup`，`R8Unorm` 整数纹理（每像素 1 字节），`textureLoad` 直接索引 |
| **渲染** | Mesh 缓存 | `CircleMeshCache` 用 `f32::to_bits()` 做 key 避免重复创建同半径圆形 Mesh |
| **物理** | 运动学接管 | Territory 侧不使用 Avian2D 物理，自实现 `KinematicVelocity` + `BulletPrevPosition` 获得完全控制权 |
| **性能** | 无锁 Profiler | `AtomicU64` 计数器/计时器，`ScopeGuard` RAII Drop 自动计时，`Mutex<Option<Instant>>` 处理帧级计时 |
| **可靠性** | 弹珠防卡死 | 检测 1.5 秒内位移 <0.8 且速度 <5.0 的弹珠，施加向上 420 速度 + 随机水平偏移 |

---

## 四、需要强化的 Rust 语法点

### 4.1 所有权与借用（必问）

项目中最关键的借用场景：

```rust
// 1. ECS Query 的 split borrowing —— 编译器保证不冲突
fn system(
    mut marbles: Query<(&mut Marble, &mut Transform, &mut LinearVelocity)>,  // 可变借用多个组件
    multiplier_zones: Query<&MultiplierZone>,                                  // 只读查询，不冲突
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,    // Without 过滤器
) { }

// 2. Res vs ResMut —— 共享 vs 独占借用
fn update_grid_render(
    mut grid: ResMut<TerritoryGrid>,     // 独占可变借用
    image_handle: Res<GridImageHandle>,  // 共享不可变借用
    mut images: ResMut<Assets<Image>>,   // 独占可变借用（不同资源不冲突）
) { }
```

**面试要点**：能解释为什么 Bevy 的 Query 可以同时借用多个可变组件（因为它们是不同 Column/Archetype 中的数据，不违反借用规则）；`Res<T>` 和 `ResMut<T>` 的区别；`Commands` 是延迟执行的（deferred mutation）。

### 4.2 生命周期

```rust
// grid.rs: 零分配迭代器的生命周期
pub fn iter_dirty_tiles(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
    // '_ 等价于 &self 的生命周期
    self.dirty_tiles.iter().enumerate().flat_map(|...| {
        std::iter::from_fn(move || { ... })  // move 捕获所有权
    })
}

// systems.rs: 空间索引查询返回引用的生命周期
fn query_nearby<'a>(&'a self, pos: Vec2, query_radius: f32)
    -> impl Iterator<Item = &'a CollisionEntry>
{ ... }

// profiler.rs: ScopeGuard 持有对 Profiler 的引用
pub struct ScopeGuard<'a> {
    profiler: &'a Profiler,
    id: ScopeId,
    start: Option<Instant>,
}
```

**面试要点**：`impl Trait` 返回值的生命周期、`'_` 匿名生命周期、`'static` 的含义、`ScopeGuard` 的 `Drop` 如何保证资源释放。

### 4.3 原子操作与内部可变性

```rust
// Profiler 被 Res<Profiler> 只读借用，但内部需要修改
pub struct Profiler {
    enabled: AtomicBool,                          // 原子布尔
    frame_accounted_ns: AtomicU64,                // 原子计数器
    scopes: [ScopeStats; ScopeId::COUNT],          // 数组长度由关联常量决定
    counters: [AtomicU64; CounterId::COUNT],
    frame_update_start: std::sync::Mutex<Option<Instant>>,  // 需要内部可变性的瞬时值
}

// ScopeGuard 的 RAII Drop
impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        let Some(start) = self.start.take() else { return; };
        self.profiler.record(self.id, start.elapsed());
    }
}
```

**面试要点**：`AtomicU64` vs `Mutex<u64>` 的场景选择（无锁 vs 有锁）、`Ordering::Relaxed` 的适用条件（不依赖 happens-before 关系时足够）、为什么 `Instant` 不能用 `Atomic`（它不满足 `Atomic` trait 的 `Copy + Eq` 约束）、RAII 模式在 Rust 中的分析器计时应用。

### 4.4 位操作

```rust
// 位掩码构造
let start_mask = u64::MAX << start_bit;
let end_mask = if end_bit == 63 { u64::MAX } else { (1u64 << (end_bit + 1)) - 1 };

// Brian Kernighan 算法：O(popcount) 遍历置位
let mut holes = (!team_bits) & mask;
while holes != 0 {
    let bit = holes.trailing_zeros() as usize;
    holes &= holes - 1;   // 清除最低位 1
    // ... 处理 bit
}
```

**面试要点**：`trailing_zeros()` 使用 CPU 的 `TZCNT` 指令（单周期）、`w &= w - 1` 的正确性证明、位操作在游戏编程中的常见场景（碰撞掩码、脏标记、状态机）。

### 4.5 枚举与模式匹配

```rust
// 带数据的枚举
pub enum ActionType { BigBall, Shield, MachineGun, CIWS }
pub enum CiwsDistanceMetric { Manhattan, EuclideanSquared }

// 枚举方法
impl TeamColor {
    pub fn index(&self) -> usize {
        match self {
            TeamColor::Red => 0,
            TeamColor::Blue => 1,   // 编译器强制穷举
            // ...
        }
    }
}

// #[repr(u16)] 确保内存布局可控（用于数组索引）
#[repr(u16)]
pub enum ScopeId { TerritorySpawnUnitsFromEvents = 0, ... }
```

**面试要点**：Rust enum 是 tagged union（比 C enum 更强）、`Option<T>` 本质是 `enum Option<T> { None, Some(T) }`、`#[repr]` 的作用、match 穷举检查、if-let / while-let 语法糖。

### 4.6 Trait 与泛型

```rust
// 自定义 Component（本质是实现 Component trait + marker 派生）
#[derive(Component, Debug, Clone)]
pub struct BigBall { pub team: TeamColor, pub size: u64 }

// Material2d 的 AsBindGroup 派生（宏生成 GPU bind group 布局代码）
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GridMaterial {
    #[texture(0, sample_type = "float")]
    pub grid_texture: Handle<Image>,
}

// 泛型 Query
fn spawn_units_from_events(
    mut action_events: MessageReader<ActionEvent>,  // 泛型约束事件类型
    ...
)
```

**面试要点**：`Component`/`Resource`/`Plugin` trait 的设计模式、派生宏 `#[derive(Component)]` 的原理（proc-macro 自动生成 impl）、泛型参数的类型推断、`impl Trait` vs `dyn Trait` 的选择。

### 4.7 迭代器与闭包

```rust
// flat_map 嵌套迭代器展平
(min_cy..=max_cy).flat_map(move |y| {
    (min_cx..=max_cx).flat_map(move |x| {
        self.buckets[self.bucket_index(x, y)].iter()
    })
})

// move 闭包捕获所有权
std::iter::from_fn(move || { ... })

// collect 类型推断
let shield_data: Vec<_> = shields.iter()
    .map(|(shield, pos)| (pos.0, shield.radius))
    .collect();
```

**面试要点**：闭包的三种 trait（`Fn`/`FnMut`/`FnOnce`）、`move` 关键字的作用、迭代器的惰性求值、`collect` 的 turbofish 语法 (`::<Vec<_>>`)、零开销抽象。

### 4.8 模块系统

```text
src/
├── main.rs          → mod colors; mod events; mod pinball; mod territory; pub mod profiler;
├── pinball/
│   ├── mod.rs        → mod components; mod layout; mod plugin; mod systems; mod utils;
│   │                   pub use plugin::PinballPlugin;
│   │                   pub use layout::{PINBALL_HEIGHT, PINBALL_WIDTH};
```

**面试要点**：`mod` 声明 vs `use` 导入、`pub` 可见性控制、`pub use` 重导出（re-export）改变模块对外 API、2018 edition 后的路径清晰度 (`crate::` 前缀)。

---

## 五、面试可能被追问的问题及应对话术

| 问题 | 应对方向 |
|------|----------|
| "为什么 Territory 不用 Avian2D 物理而自己写运动学？" | 需要完全控制碰撞逻辑（连续碰撞检测、自定义边界反弹、大球占领轨迹追踪），Avian2D 物理引擎的碰撞回调粒度不够细。同时避免了物理引擎内部锁争用。 |
| "1024×1024 的网格每帧更新不会很慢吗？" | 用了三层优化：(1) 脏 Tile 增量上传，大多数帧只更新 ≤256 个 tile；(2) 位图行加速，`row_all_team` 用 u64 位掩码 O(row_words) 判断全属于；(3) R8Unorm 纹理格式每像素仅 1 字节，全量也只有 1MB。 |
| "为什么用 AtomicU64 而不是 Mutex？" | Profiler 在每帧的多个系统中被并发读取（都是 `Res<Profiler>`），`AtomicU64` 的 `Relaxed` 顺序在累加计数器时足够——不需要跨计数器的 happens-before 保证，且无锁避免了 Mutex 在热路径上的争用开销。`Mutex<Option<Instant>>` 只在帧开始/结束时各获取一次。 |
| "Brian Kernighan 算法的时间复杂度？" | O(popcount)，即只遍历置位的 bit 而非遍历全部 64 位。项目中用它遍历非己方格子（空洞），实际场景中空洞通常很少。 |
| "ECS 架构比 OOP 好在哪？" | 数据与行为分离：Component 是纯数据，System 是纯函数。同一个实体可以挂任意组合的 Component 而无需改类继承树。Query 自动做 archetype 过滤——Bevy 只会迭代符合条件的实体。多线程友好：Bevy 自动根据 Query 的读写访问做并行调度。 |

---

## 六、项目简历上的卖点排序

1. **ECS 架构实践** — 证明你对非 OOP 设计模式的理解（这是 Rust 游戏开发的标志性范式）
2. **位图 + 位操作优化** — 证明你有底层性能意识
3. **空间索引（均匀网格）** — 证明你做过碰撞检测/空间查询优化
4. **CPU-GPU 数据传输优化** — 脏 Tile 增量上传 + 自定义 Shader
5. **坐标系统隔离设计** — 证明你有架构思维
6. **自研 Profiler（无锁 + RAII）** — 证明你掌握并发原语和资源管理模式

面试时重点展开前三项即可，后三项作为深度追问的弹药。如果时间有限，**位图领土网格 + Brian Kernighan 算法** 是最出彩的单点——既有数学美感，又有实实在在的性能收益。
