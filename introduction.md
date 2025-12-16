# BevyMarble 项目 Rust 语法入门指南

本文档面向没有 Rust 基础的读者，介绍 BevyMarble 项目中使用的所有 Rust 语法知识。

---

## 目录

1. [基础语法](#1-基础语法)
2. [所有权与借用](#2-所有权与借用)
3. [枚举与模式匹配](#3-枚举与模式匹配)
4. [结构体与方法](#4-结构体与方法)
5. [Trait（特征）](#5-trait特征)
6. [泛型](#6-泛型)
7. [闭包与迭代器](#7-闭包与迭代器)
8. [模块系统](#8-模块系统)
9. [宏](#9-宏)
10. [Option 与 Result](#10-option-与-result)
11. [Bevy ECS 模式](#11-bevy-ecs-模式)

---

## 1. 基础语法

### 1.1 变量声明

Rust 使用 `let` 声明变量，默认不可变：

```rust
// 不可变变量
let x = 5;

// 可变变量（使用 mut 关键字）
let mut counter = 0;
counter += 1;  // 允许修改
```

项目中的例子：
```rust
// pinball/systems.rs
let mut closest: Option<(Vec2, f32)> = None;  // 可变变量
let radius = calculate_radius(marble.value);   // 不可变变量
```

### 1.2 常量

使用 `const` 声明编译时常量，必须显式标注类型：

```rust
// pinball/layout.rs
pub const PINBALL_WIDTH: f32 = 400.0;
pub const PINBALL_HEIGHT: f32 = 800.0;
pub const WALL_THICKNESS: f32 = 10.0;

// territory/coords.rs
pub const TERRITORY_LOGIC_WIDTH: f32 = 1024.0;
pub const TERRITORY_LOGIC_HEIGHT: f32 = 1024.0;
```

### 1.3 基本数据类型

```rust
// 整数类型
let a: i32 = -42;        // 有符号 32 位整数
let b: u32 = 42;         // 无符号 32 位整数
let c: u64 = 1000000;    // 无符号 64 位整数
let d: usize = 100;      // 平台相关的无符号整数（用于索引）

// 浮点数
let e: f32 = 3.14;       // 32 位浮点数
let f: f64 = 3.141592;   // 64 位浮点数

// 布尔值
let g: bool = true;

// 字符串
let h: &str = "hello";           // 字符串切片
let i: String = "hello".to_string();  // 堆分配字符串
```

### 1.4 类型转换（as 关键字）

Rust 是强类型语言，需要显式转换：

```rust
// territory/grid.rs
let total_cells = (width * height) as usize;

// pinball/utils.rs
let ratio = (value as f32).log10() / (MAX_VALUE as f32).log10();

// territory/systems.rs
let grid_x = x as u32;
```

### 1.5 元组

元组可以存储多个不同类型的值：

```rust
// 声明元组
let point: (f32, f32) = (10.0, 20.0);

// 访问元组元素
let x = point.0;  // 10.0
let y = point.1;  // 20.0

// 项目实例 - colors.rs
pub fn start_corner(&self) -> (u32, u32) {
    match self {
        TeamColor::Red => (0, 0),
        TeamColor::Blue => (1023, 0),
        TeamColor::Green => (0, 1023),
        TeamColor::Yellow => (1023, 1023),
    }
}
```

### 1.6 数组

固定长度的同类型元素集合：

```rust
// 声明数组，类型为 [元素类型; 长度]
let colors: [TeamColor; 4] = [
    TeamColor::Red,
    TeamColor::Blue,
    TeamColor::Green,
    TeamColor::Yellow
];

// 初始化所有元素为相同值
let team_counts = [0u32; 4];  // [0, 0, 0, 0]

// 访问元素
let first = colors[0];
```

### 1.7 Vec（动态数组）

可变长度的集合：

```rust
// 创建空 Vec
let mut points = Vec::new();
points.push((x, y));

// 使用宏创建并初始化
let mut cells = vec![None; total_cells];

// 从迭代器收集
let shield_data: Vec<_> = shields.iter()
    .map(|(shield, pos)| (pos.0, shield.radius))
    .collect();
```

### 1.8 控制流

#### if/else 表达式

```rust
// 基本用法
if value > 100 {
    println!("large");
} else if value > 10 {
    println!("medium");
} else {
    println!("small");
}

// if 作为表达式返回值
let offset = if start_x == 0 { 50.0 } else { -50.0 };
```

#### for 循环

```rust
// 遍历范围
for i in 0..5 {           // 0, 1, 2, 3, 4
    println!("{}", i);
}

for i in 0..=5 {          // 0, 1, 2, 3, 4, 5（包含结束值）
    println!("{}", i);
}

// 遍历迭代器
for event in collision_started.read() {
    // 处理事件
}

// 遍历并获取可变引用
for (marble, mut transform) in marbles.iter_mut() {
    transform.translation.x += 1.0;
}
```

#### loop 循环

```rust
// 无限循环，用 break 退出
loop {
    points.push((x, y));
    if x == x1 && y == y1 {
        break;  // 退出循环
    }
    // 继续执行...
}
```

#### continue 与 break

```rust
for event in events.read() {
    if !valid_event(event) {
        continue;  // 跳过本次迭代
    }
    if should_stop(event) {
        break;     // 退出循环
    }
    process(event);
}
```

### 1.9 字符串格式化

```rust
// 使用 format! 宏
let text = format!("{}B", value / 1_000_000_000);  // "32B"
let text = format!("{}M", value / 1_000_000);      // "1000M"
let text = format!("{}K", value / 1_000);          // "500K"
let text = format!("{}", value);                    // "123"

// 完整的格式化函数示例 - pinball/utils.rs
pub fn format_value(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{}B", value / 1_000_000_000)
    } else if value >= 1_000_000 {
        format!("{}M", value / 1_000_000)
    } else if value >= 1_000 {
        format!("{}K", value / 1_000)
    } else {
        format!("{}", value)
    }
}
```

---

## 2. 所有权与借用

Rust 最核心的概念是所有权系统，它在编译时保证内存安全。

### 2.1 所有权规则

1. Rust 中每个值都有一个所有者
2. 同一时间只能有一个所有者
3. 当所有者离开作用域，值被丢弃

```rust
{
    let s = String::from("hello");  // s 拥有这个 String
    // 使用 s
}  // s 离开作用域，内存被释放
```

### 2.2 移动（Move）

非 Copy 类型赋值时会发生所有权转移：

```rust
let s1 = String::from("hello");
let s2 = s1;  // 所有权从 s1 移动到 s2
// println!("{}", s1);  // 错误！s1 不再有效
println!("{}", s2);     // 可以
```

### 2.3 引用与借用

使用 `&` 创建引用，借用值而不获取所有权：

```rust
// 不可变引用
fn calculate_length(s: &String) -> usize {
    s.len()
}

let s = String::from("hello");
let len = calculate_length(&s);  // 借用 s
println!("{}", s);  // s 仍然有效

// 可变引用
fn append_world(s: &mut String) {
    s.push_str(" world");
}

let mut s = String::from("hello");
append_world(&mut s);  // 可变借用
```

### 2.4 项目中的借用示例

```rust
// pinball/layout.rs - 函数参数借用
fn spawn_wall(
    commands: &mut Commands,           // 可变借用
    meshes: &mut ResMut<Assets<Mesh>>, // 可变借用
    material: Handle<ColorMaterial>,   // 所有权转移（因为是 Copy 类型）
    position: Vec2,                    // Copy 类型，复制
    size: Vec2,                        // Copy 类型，复制
)

// territory/systems.rs - 查询系统中的借用
pub fn update_grid_render(
    grid: Res<TerritoryGrid>,          // 共享引用
    mut images: ResMut<Assets<Image>>, // 独占可变引用
)
```

### 2.5 Copy 与 Clone

实现 `Copy` trait 的类型赋值时会复制而非移动：

```rust
// 基本类型都是 Copy
let x: i32 = 5;
let y = x;  // 复制，x 仍有效

// 自定义 Copy 类型
#[derive(Clone, Copy)]
pub struct LogicPosition(pub Vec2);

let pos1 = LogicPosition(Vec2::new(0.0, 0.0));
let pos2 = pos1;  // 复制，pos1 仍有效
```

`Clone` 需要显式调用：

```rust
let s1 = String::from("hello");
let s2 = s1.clone();  // 显式克隆
println!("{} {}", s1, s2);  // 都有效
```

---

## 3. 枚举与模式匹配

### 3.1 枚举定义

枚举（enum）表示一个类型可以是几个变体之一：

```rust
// colors.rs - 队伍颜色枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component, Default)]
pub enum TeamColor {
    #[default]  // 指定默认变体
    Red,
    Blue,
    Green,
    Yellow,
}

// events.rs - 动作类型枚举
pub enum ActionType {
    BigBall,
    Shield,
    MachineGun,
    CIWS,
}

// territory/components.rs - 物理层枚举
#[derive(PhysicsLayer, Default, Clone, Copy, Debug)]
pub enum GameLayer {
    #[default]
    Default,
    RedTeam,
    BlueTeam,
    GreenTeam,
    YellowTeam,
}
```

### 3.2 match 表达式

`match` 必须穷尽所有可能的情况：

```rust
// colors.rs
impl TeamColor {
    pub fn to_color(&self) -> Color {
        match self {
            TeamColor::Red => Color::srgb(0.9, 0.2, 0.2),
            TeamColor::Blue => Color::srgb(0.2, 0.4, 0.9),
            TeamColor::Green => Color::srgb(0.2, 0.8, 0.3),
            TeamColor::Yellow => Color::srgb(0.9, 0.85, 0.2),
        }
    }

    pub fn start_corner(&self) -> (u32, u32) {
        match self {
            TeamColor::Red => (0, 0),
            TeamColor::Blue => (1023, 0),
            TeamColor::Green => (0, 1023),
            TeamColor::Yellow => (1023, 1023),
        }
    }
}

// territory/systems.rs - 匹配处理不同动作
match event.action_type {
    ActionType::BigBall => { spawn_bigball(...) }
    ActionType::Shield => { spawn_shield(...) }
    ActionType::MachineGun => { spawn_machine_gun(...) }
    ActionType::CIWS => { spawn_ciws(...) }
}
```

### 3.3 if let 语法糖

当只关心一种情况时，`if let` 比 `match` 更简洁：

```rust
// territory/systems.rs
if let Ok((hq, _)) = hqs.get(target_entity) {
    if hq.team != bullet.team {
        // 处理逻辑
    }
}

if let Some((target_pos, _)) = closest {
    // 处理找到最近目标的情况
}

// 复杂模式匹配
if let (Some((x0, y0)), Some((x1, y1))) =
    (grid.logic_to_grid(last_pos), grid.logic_to_grid(current_pos))
{
    // 两个坐标都有效时执行
}
```

### 3.4 解构

在模式匹配中解构元组和结构体：

```rust
// 解构元组
for (y, x_positions) in rows.iter() {
    for x in x_positions.iter() {
        spawn_peg(y, x);
    }
}

// 解构组件查询结果
for (entity, marble, transform) in marbles.iter() {
    // entity, marble, transform 分别被解构出来
}

// 在 if let 中解构
if let Ok([(mut ball1, mut vel1, mass1), (mut ball2, mut vel2, mass2)]) =
    bigballs.get_many_mut([event.collider1, event.collider2])
{
    // 处理两个大球的碰撞
}
```

---

## 4. 结构体与方法

### 4.1 结构体定义

```rust
// 普通结构体
#[derive(Component, Debug, Clone)]
pub struct Marble {
    pub team: TeamColor,
    pub value: u64,
}

#[derive(Component, Debug, Clone)]
pub struct BigBall {
    pub team: TeamColor,
    pub size: u64,
}

#[derive(Component, Debug, Clone)]
pub struct MachineGun {
    pub team: TeamColor,
    pub bullets: u64,
    pub fire_timer: Timer,
    pub rotation: f32,
    pub rotation_speed: f32,
}

// 元组结构体（Newtype 模式）
#[derive(Component, Debug, Clone, Copy)]
pub struct LogicPosition(pub Vec2);  // 包装 Vec2

#[derive(Component, Debug, Clone, Copy)]
pub struct LastLogicPosition(pub Vec2);

// 带关联实体的结构体
#[derive(Component, Debug, Clone, Copy)]
pub struct MarbleText {
    pub marble_entity: Entity,
}

// Resource 结构体
#[derive(Resource, Debug)]
pub struct TerritoryGrid {
    cells: Vec<Option<TeamColor>>,
    pub width: u32,
    pub height: u32,
}
```

### 4.2 impl 块与方法

在 `impl` 块中定义方法：

```rust
// territory/grid.rs
impl TerritoryGrid {
    // 关联函数（类似静态方法），使用 Self 返回类型
    pub fn new(width: u32, height: u32) -> Self {
        let total_cells = (width * height) as usize;
        Self {
            cells: vec![None; total_cells],
            width,
            height,
        }
    }

    // 实例方法，&self 表示不可变借用
    pub fn get(&self, x: u32, y: u32) -> Option<TeamColor> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y * self.width + x) as usize;
        self.cells[index]
    }

    // 可变方法，&mut self 表示可变借用
    pub fn set(&mut self, x: u32, y: u32, team: Option<TeamColor>) {
        if x < self.width && y < self.height {
            let index = (y * self.width + x) as usize;
            self.cells[index] = team;
        }
    }

    // 复杂逻辑方法
    pub fn occupy(&mut self, x: u32, y: u32, team: TeamColor, radius: u32) -> bool {
        // 实现占领逻辑
    }

    // 使用闭包的方法
    pub fn count_territory(&self, team: TeamColor) -> u32 {
        self.cells
            .iter()
            .filter(|cell| **cell == Some(team))
            .count() as u32
    }
}
```

### 4.3 Marble 结构体方法示例

```rust
// pinball/components.rs
impl Marble {
    pub fn new(team: TeamColor) -> Self {
        Self { team, value: 1 }
    }

    pub fn multiply(&mut self, multiplier: u64) {
        self.value = self.value
            .saturating_mul(multiplier)  // 防溢出乘法
            .min(MAX_VALUE);             // 上限限制
    }
}
```

---

## 5. Trait（特征）

Trait 定义共享行为，类似其他语言的接口。

### 5.1 标准库 Trait

项目中使用的常见派生 trait：

```rust
#[derive(
    Debug,       // 允许 {:?} 格式化输出
    Clone,       // 允许 .clone() 方法
    Copy,        // 允许值复制（赋值时）
    PartialEq,   // 允许 == 和 != 比较
    Eq,          // 完全相等性（比 PartialEq 更严格）
    Hash,        // 允许作为 HashMap 的键
    Default,     // 提供默认值
)]
pub enum TeamColor { ... }
```

### 5.2 Bevy 框架 Trait

```rust
// Component - 标记为 ECS 组件
#[derive(Component)]
pub struct Marble { ... }

// Resource - 标记为全局资源
#[derive(Resource)]
pub struct TerritoryGrid { ... }

// Message - 标记为事件消息（来自 Avian2D）
#[derive(Message)]
pub struct ActionEvent { ... }
```

### 5.3 实现 Trait

为类型实现自定义 trait：

```rust
// pinball/plugin.rs - 实现 Plugin trait
pub struct PinballPlugin;

impl Plugin for PinballPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_pinball_layout)
           .add_systems(Startup, spawn_initial_marbles.after(spawn_pinball_layout))
           .add_systems(Update, (
               check_multiplier_collision,
               check_action_zone_collision,
               contain_marbles,
               update_marble_display,
               sync_marble_text_position,
           ));
    }
}

// territory/plugin.rs
impl Plugin for TerritoryPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerritoryGrid::new(1024, 1024))
           .add_systems(Startup, (setup_grid_render, setup_initial_game))
           .add_systems(Update, (
               handle_action_events,
               update_grid_render,
               // ... 更多系统
           ));
    }
}
```

### 5.4 Default Trait

提供类型的默认值：

```rust
// 使用 #[default] 属性指定枚举默认变体
#[derive(Default)]
pub enum TeamColor {
    #[default]
    Red,
    Blue,
    Green,
    Yellow,
}

// 使用 ..default() 语法填充其他字段
TextFont {
    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
    font_size: 14.0,
    ..default()  // 其他字段使用默认值
}
```

---

## 6. 泛型

泛型允许编写适用于多种类型的代码。

### 6.1 Bevy Query 中的泛型

Query 是 Bevy 中最常用的泛型类型：

```rust
// 基础查询
Query<&Marble>                    // 查询 Marble 组件的不可变引用
Query<&mut Transform>             // 查询 Transform 的可变引用

// 多组件查询
Query<(&Marble, &Transform)>      // 同时查询两个组件
Query<(&mut Marble, &mut Transform, &mut LinearVelocity)>

// 带过滤器的查询
Query<&Marble, With<BigBall>>     // 只查询有 BigBall 组件的实体
Query<&Transform, Without<Marble>> // 排除有 Marble 组件的实体
Query<(&Marble, &Transform), Changed<Marble>>  // 只查询 Marble 变化的实体
```

### 6.2 Query 过滤器

```rust
// With<T> - 要求实体必须有该组件
Query<(Entity, &Transform), With<Marble>>

// Without<T> - 要求实体没有该组件
Query<(&PinballSpawnPoint, &Transform), Without<Marble>>
Query<(&MarbleText, &mut Transform), Without<Marble>>

// Changed<T> - 只匹配组件发生变化的实体
Query<(&Marble, &Transform), Changed<Marble>>

// Or<T> - 匹配满足任一条件的实体
Query<(&TerritoryUnit, &Transform), Or<(With<BigBall>, With<Bullet>)>>
```

### 6.3 完整的系统函数泛型示例

```rust
pub fn check_multiplier_collision(
    // 事件读取器，泛型参数是事件类型
    mut collision_started: MessageReader<CollisionStart>,

    // 带多个组件的可变查询
    mut marbles: Query<(&mut Marble, &mut Transform, &mut LinearVelocity)>,

    // 只读查询
    multiplier_zones: Query<&MultiplierZone>,

    // 带过滤器的查询
    spawn_points: Query<(&PinballSpawnPoint, &Transform), Without<Marble>>,
) {
    // 系统逻辑
}
```

---

## 7. 闭包与迭代器

### 7.1 闭包语法

闭包是可以捕获环境变量的匿名函数：

```rust
// 基本闭包语法: |参数| 表达式
let add = |a, b| a + b;
let result = add(2, 3);  // 5

// 带类型标注
let multiply = |x: i32, y: i32| -> i32 { x * y };

// 捕获环境变量
let team = TeamColor::Red;
let is_same_team = |other: TeamColor| other == team;
```

### 7.2 迭代器方法

项目中大量使用迭代器链：

```rust
// territory/grid.rs - filter + count
pub fn count_territory(&self, team: TeamColor) -> u32 {
    self.cells
        .iter()                              // 创建迭代器
        .filter(|cell| **cell == Some(team)) // 过滤
        .count() as u32                      // 计数
}

// territory/systems.rs - map + collect
let shield_data: Vec<_> = shields.iter()
    .map(|(shield, logic_pos)| (logic_pos.0, shield.radius, shield.team))
    .collect();

// enumerate + filter + map + collect
let alive_teams: Vec<_> = team_counts.iter()
    .enumerate()                              // 添加索引
    .filter(|&(_, count)| *count > 0)        // 过滤非零
    .map(|(i, _)| i)                          // 只保留索引
    .collect();
```

### 7.3 常用迭代器方法

```rust
// .iter() - 创建不可变引用迭代器
for marble in marbles.iter() { ... }

// .iter_mut() - 创建可变引用迭代器
for (entity, mut transform) in entities.iter_mut() { ... }

// .map() - 转换元素
let values: Vec<_> = items.iter().map(|x| x * 2).collect();

// .filter() - 过滤元素
let positive: Vec<_> = numbers.iter().filter(|&&x| x > 0).collect();

// .find() - 查找第一个匹配元素
let found = items.iter().find(|x| x.id == target_id);

// .any() - 检查是否有任一元素满足条件
let has_red = teams.iter().any(|t| *t == TeamColor::Red);

// .collect() - 收集为集合
let vec: Vec<_> = iter.collect();
```

### 7.4 for 循环与迭代器

```rust
// 直接遍历
for event in collision_started.read() {
    process(event);
}

// 遍历带索引
for (index, item) in items.iter().enumerate() {
    println!("{}: {:?}", index, item);
}

// 遍历查询结果
for (entity, marble, transform) in marbles.iter() {
    // 处理每个实体
}
```

---

## 8. 模块系统

### 8.1 模块声明

```rust
// main.rs - 声明子模块
mod colors;      // 加载 colors.rs 或 colors/mod.rs
mod events;      // 加载 events.rs
mod pinball;     // 加载 pinball/mod.rs
mod territory;   // 加载 territory/mod.rs
```

### 8.2 子模块组织

```rust
// pinball/mod.rs
mod components;  // 私有子模块
mod layout;
mod plugin;
mod systems;
mod utils;

// 重导出公开 API
pub use plugin::PinballPlugin;
```

### 8.3 use 导入

```rust
// 导入单个项
use colors::TeamColor;

// 导入多个项
use events::{ActionEvent, UnitDestroyedEvent, VictoryEvent};

// 导入所有公开项（慎用）
use bevy::prelude::*;

// 带别名导入
use std::collections::HashMap as Map;
```

### 8.4 可见性

```rust
// pub - 公开，外部可访问
pub struct PinballPlugin;
pub const WALL_THICKNESS: f32 = 10.0;
pub fn format_value(value: u64) -> String { ... }

// 默认 - 私有，仅当前模块可访问
struct InternalData;
fn helper_function() { ... }

// pub(crate) - 仅 crate 内可访问
pub(crate) fn internal_api() { ... }
```

### 8.5 项目模块结构

```
src/
├── main.rs              // crate 根，声明所有顶层模块
├── colors.rs            // 颜色定义模块
├── events.rs            // 事件定义模块
├── pinball/
│   ├── mod.rs           // 模块入口，声明子模块
│   ├── components.rs    // 组件定义
│   ├── systems.rs       // 系统逻辑
│   ├── layout.rs        // 布局生成
│   ├── plugin.rs        // Bevy 插件
│   └── utils.rs         // 工具函数
└── territory/
    ├── mod.rs
    ├── components.rs
    ├── systems.rs
    ├── grid.rs
    ├── coords.rs
    ├── setup.rs
    ├── render.rs
    └── plugin.rs
```

---

## 9. 宏

### 9.1 派生宏（Derive Macros）

自动实现 trait 的宏：

```rust
// 标准库派生宏
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TeamColor { ... }

// Bevy 派生宏
#[derive(Component)]   // 使类型可作为 ECS 组件
#[derive(Resource)]    // 使类型可作为全局资源

// Avian2D 派生宏
#[derive(Message)]     // 使类型可作为事件消息
#[derive(PhysicsLayer)] // 定义物理碰撞层
```

### 9.2 属性宏

```rust
// #[default] - 指定枚举默认变体
#[derive(Default)]
pub enum TeamColor {
    #[default]
    Red,
    Blue,
    Green,
    Yellow,
}
```

### 9.3 函数式宏

```rust
// vec! - 创建 Vec
let cells = vec![None; total_cells];
let points = vec![(0, 0), (1, 1), (2, 2)];

// format! - 格式化字符串
let text = format!("Score: {}", score);

// println! - 打印到控制台（调试用）
println!("Debug: {:?}", value);
```

---

## 10. Option 与 Result

### 10.1 Option 类型

表示值可能存在或不存在：

```rust
// Option 定义
enum Option<T> {
    Some(T),  // 有值
    None,     // 无值
}

// 返回 Option 的函数
pub fn get(&self, x: u32, y: u32) -> Option<TeamColor> {
    if x >= self.width || y >= self.height {
        return None;
    }
    let index = (y * self.width + x) as usize;
    self.cells[index]  // 返回 Some(color) 或 None
}

pub fn logic_to_grid(&self, logic_pos: Vec2) -> Option<(u32, u32)> {
    if logic_pos.x < 0.0 || logic_pos.y < 0.0 {
        return None;
    }
    Some((grid_x, grid_y))
}
```

### 10.2 处理 Option

```rust
// if let - 只处理 Some 情况
if let Some(color) = grid.get(x, y) {
    process(color);
}

// match - 处理所有情况
match grid.get(x, y) {
    Some(color) => process(color),
    None => default_action(),
}

// unwrap - 假设一定是 Some（可能 panic）
let value = option.unwrap();

// unwrap_or - 提供默认值
let value = option.unwrap_or(default);

// is_some / is_none - 检查状态
if closest.is_none() {
    closest = Some((target_pos, distance));
}
```

### 10.3 Result 类型

表示操作可能成功或失败：

```rust
// Result 定义
enum Result<T, E> {
    Ok(T),   // 成功
    Err(E),  // 失败
}

// 项目中的使用示例
if let Ok((mut marble, mut transform, mut velocity)) =
    marbles.get_mut(marble_entity)
{
    // 成功获取组件
    marble.multiply(zone.multiplier);
}

// get_many_mut 返回 Result
if let Ok([(mut ball1, mut vel1, mass1), (mut ball2, mut vel2, mass2)]) =
    bigballs.get_many_mut([event.collider1, event.collider2])
{
    // 成功获取两个实体
}
```

---

## 11. Bevy ECS 模式

### 11.1 ECS 概念

- **Entity（实体）**：游戏对象的唯一标识符
- **Component（组件）**：附加到实体的数据
- **System（系统）**：处理组件的逻辑函数
- **Resource（资源）**：全局共享数据

### 11.2 组件定义

```rust
// 数据组件 - 存储数据
#[derive(Component, Debug, Clone)]
pub struct Marble {
    pub team: TeamColor,
    pub value: u64,
}

// 标记组件 - 仅用于标识
#[derive(Component, Debug, Clone, Copy)]
pub struct PinballWall;

#[derive(Component, Debug, Clone, Copy)]
pub struct TerritoryUnit {
    pub team: TeamColor,
}
```

### 11.3 生成实体

```rust
// 使用 Commands 生成实体并添加组件
let entity = commands.spawn((
    Marble::new(team),              // 自定义组件
    RigidBody::Dynamic,             // 物理组件
    Collider::circle(radius),       // 碰撞体
    Restitution::new(0.6),          // 弹性
    LinearVelocity(velocity),       // 速度
    Transform::from_translation(pos.extend(0.5)),  // 变换
    Mesh2d(mesh),                   // 网格
    MeshMaterial2d(material),       // 材质
    CollisionEventsEnabled,         // 启用碰撞事件
)).id();  // 获取实体 ID

// 添加子实体
commands.spawn((
    MarbleText { marble_entity: entity },
    Text2d::new("1"),
    TextFont { font, font_size: 14.0, ..default() },
    Transform::from_translation(pos.extend(1.0)),
));
```

### 11.4 系统函数

系统函数通过参数声明需要的资源和查询：

```rust
pub fn update_marble_display(
    // 命令队列
    mut commands: Commands,

    // 资产管理
    mut meshes: ResMut<Assets<Mesh>>,

    // 带变化检测的查询
    marbles: Query<(Entity, &Marble, &Transform), Changed<Marble>>,

    // 文本查询
    mut text_query: Query<(&MarbleText, &mut Text2d)>,
) {
    for (entity, marble, transform) in marbles.iter() {
        // 更新逻辑
    }
}
```

### 11.5 资源

```rust
// 定义资源
#[derive(Resource, Debug)]
pub struct TerritoryGrid {
    cells: Vec<Option<TeamColor>>,
    pub width: u32,
    pub height: u32,
}

// 注册资源
app.insert_resource(TerritoryGrid::new(1024, 1024));

// 在系统中访问资源
pub fn update_grid_render(
    grid: Res<TerritoryGrid>,           // 只读访问
    mut images: ResMut<Assets<Image>>,  // 可变访问
) {
    if !grid.is_changed() {
        return;  // 资源未变化，跳过
    }
    // 更新逻辑
}
```

### 11.6 事件系统

```rust
// 定义事件
#[derive(Message, Debug, Clone)]
pub struct ActionEvent {
    pub team: TeamColor,
    pub action_type: ActionType,
    pub value: u64,
}

// 注册事件
app.add_message::<ActionEvent>();

// 发送事件
pub fn check_action_zone_collision(
    mut action_events: MessageWriter<ActionEvent>,
) {
    action_events.write(ActionEvent {
        team: marble.team,
        action_type: ActionType::BigBall,
        value: marble.value,
    });
}

// 读取事件
pub fn handle_action_events(
    mut events: MessageReader<ActionEvent>,
) {
    for event in events.read() {
        match event.action_type {
            ActionType::BigBall => spawn_bigball(event),
            ActionType::Shield => spawn_shield(event),
            // ...
        }
    }
}
```

### 11.7 插件

将相关功能组织为插件：

```rust
pub struct PinballPlugin;

impl Plugin for PinballPlugin {
    fn build(&self, app: &mut App) {
        app
            // 启动时执行一次
            .add_systems(Startup, spawn_pinball_layout)
            .add_systems(Startup, spawn_initial_marbles.after(spawn_pinball_layout))

            // 每帧执行
            .add_systems(Update, (
                check_multiplier_collision,
                check_action_zone_collision,
                contain_marbles,
                update_marble_display,
                sync_marble_text_position,
            ));
    }
}

// 主程序中使用插件
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PinballPlugin)
        .add_plugins(TerritoryPlugin)
        .run();
}
```

### 11.8 系统调度

```rust
// 基本添加
.add_systems(Update, my_system)

// 多个系统（并行执行）
.add_systems(Update, (system_a, system_b, system_c))

// 顺序执行
.add_systems(Startup, spawn_initial_marbles.after(spawn_pinball_layout))
```

---

## 附录：项目中的数学运算

### 向量运算

```rust
// 创建向量
let pos = Vec2::new(x, y);
let velocity = Vec2::NEG_Y * 490.0;

// 向量运算
let direction = (target - origin).normalize();  // 归一化
let distance = pos1.distance(pos2);             // 距离
let length = velocity.length();                  // 长度

// 三维扩展
let pos_3d = pos_2d.extend(z);                  // Vec2 -> Vec3
let pos_2d = pos_3d.truncate();                 // Vec3 -> Vec2
```

### 三角函数

```rust
use std::f32::consts::{PI, TAU};

let angle = rng.gen_range(0.0..TAU);
let direction = Vec2::new(angle.cos(), angle.sin());

gun.rotation += gun.rotation_speed * delta;
let dir = Vec2::new(gun.rotation.cos(), gun.rotation.sin());
```

### 防溢出运算

```rust
// 防止整数溢出
value.saturating_mul(multiplier)  // 乘法，溢出返回最大值
value.saturating_sub(amount)      // 减法，下溢返回 0
value.saturating_add(amount)      // 加法，溢出返回最大值

// 范围限制
value.min(MAX_VALUE)              // 上限
value.max(MIN_VALUE)              // 下限
value.clamp(min, max)             // 同时限制上下限
```

---

## 总结

本指南涵盖了 BevyMarble 项目中使用的主要 Rust 语法特性：

1. **基础语法**：变量、类型、控制流、函数
2. **所有权系统**：Rust 的核心特性，确保内存安全
3. **枚举与模式匹配**：表达多种可能状态
4. **结构体与方法**：组织数据和行为
5. **Trait**：定义共享行为
6. **泛型**：编写灵活、可复用的代码
7. **闭包与迭代器**：函数式编程风格
8. **模块系统**：组织代码结构
9. **宏**：元编程和代码生成
10. **Option/Result**：安全的空值和错误处理
11. **Bevy ECS**：游戏引擎的核心架构

建议按照本文档的顺序逐步学习，结合项目源码理解每个概念的实际应用。
