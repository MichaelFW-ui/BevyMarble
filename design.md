# BevyMarble 设计文档（框架 / 架构 / 代码组织）

## 1. 总览

BevyMarble 是一个基于 **Bevy 0.17** 的 2D 游戏原型，整体采用 **ECS（Entity-Component-System）** 架构，并以 **Plugin（插件）** 作为功能模块边界进行组合。

游戏由两个相对独立、同屏并行运行的子玩法组成：

- **左侧弹珠机（Pinball）**：负责“随机/物理驱动”的行动选择与数值累积。
- **右侧领土战场（Territory）**：负责单位生成、战斗、占领格子、胜负判定。

两者通过共享的 **Message（消息）** 在运行时解耦通信：弹珠机产生 `ActionEvent`，领土战场消费该消息并生成对应单位；战场产生 `VictoryEvent` / `UnitDestroyedEvent` 作为对外广播。

## 2. 采用的游戏框架与编程模型

### 2.1 Bevy：ECS + Schedule + Plugin

- **Entity（实体）**：世界中的对象 ID。
- **Component（组件）**：挂在实体上的数据（如 `Marble`、`BigBall`、`Bullet`）。
- **System（系统）**：纯函数风格的逻辑单元，通过 `Query/Res/Commands` 读写世界状态。
- **Schedule（调度）**：按阶段（如 `Startup`、`Update`）与顺序运行系统。
- **Plugin（插件）**：把一组系统/资源/配置打包成可插拔模块。

工程入口在 `src/main.rs`：用 `App::new()` 组合默认插件、物理插件、资源、消息、以及业务插件。

### 2.2 Avian2D：2D 物理与碰撞事件

本项目使用 `avian2d` 提供的 2D 物理组件与碰撞消息（如 `RigidBody`、`Collider`、`LinearVelocity`、`CollisionStart` 等），并使用碰撞层 `CollisionLayers` 做敌我过滤（见 `src/territory/components.rs`）。

## 3. 代码结构（“什么功能在什么位置”）

### 3.1 顶层模块

- `src/main.rs`：应用装配（窗口、物理、重力、消息、插件注册）、相机生成。
- `src/colors.rs`：`TeamColor`（队伍枚举）、显示色、起始角落等通用逻辑。
- `src/events.rs`：跨模块通信的消息类型：
  - `ActionEvent`：弹珠机选择结果 → 驱动战场生成单位。
  - `VictoryEvent`：胜利广播。
  - `UnitDestroyedEvent`：单位被消灭广播。

### 3.2 Pinball（左侧弹珠机）

目录：`src/pinball/`

- `mod.rs`：模块导出（`pub use PinballPlugin;`）。
- `plugin.rs`：`PinballPlugin` 注册本模块系统（`Startup/Update`）。
- `components.rs`：弹珠机侧组件定义：
  - `Marble`（队伍与数值）、`MultiplierZone`（加倍区）、`ActionZone`（行动选择区）等。
- `layout.rs`：弹珠机“关卡/摆放”生成（墙、钉子、加倍区、行动区、四队出生点）。
  - 关键常量：`PINBALL_WIDTH/HEIGHT`、`PINBALL_OFFSET_X`（左侧偏移）。
- `systems.rs`：运行时逻辑：
  - 生成初始弹珠（`spawn_initial_marbles`）
  - 处理碰撞：进入加倍区翻倍（`check_multiplier_collision`）；进入行动区发送 `ActionEvent` 并重置弹珠（`check_action_zone_collision`）
  - 安全回收：离开区域则传送回出生点（`contain_marbles`）
  - UI 同步：根据数值更新弹珠大小与文本（`update_marble_display`、`sync_marble_text_position`）
- `utils.rs`：数值显示与半径映射（`format_value`、`calculate_radius`、`MAX_VALUE`）。

### 3.3 Territory（右侧领土战场）

目录：`src/territory/`

- `mod.rs`：模块导出（`pub use TerritoryPlugin;`）。
- `plugin.rs`：`TerritoryPlugin` 组合战场资源与系统：
  - 注入 `TerritoryGrid` 资源
  - `Startup`：初始化网格渲染与开局单位
  - `Update`：战斗、占领、渲染更新与胜负判定等
- `components.rs`：战场侧组件定义：
  - 单位：`BigBall`、`Shield`、`MachineGun`、`CIWS`、`HQ`、`Bullet`
  - 通用标记：`TerritoryUnit`
  - 空间：`LogicPosition`、`LastLogicPosition`
  - 物理层：`GameLayer` + `TeamColor::to_layer()/enemy_layers()`
- `grid.rs`：`TerritoryGrid` 资源（1024×1024），负责“格子归属”的读写与占领规则（含护盾半径保护判定）。
- `coords.rs`：战场**逻辑坐标** ↔ **渲染坐标** ↔ **网格索引**的转换函数与常量。
- `render.rs`：把 `TerritoryGrid` 渲染为一张 `Image`（纹理），用 `Sprite` 显示，并在网格变化时增量更新像素。
- `setup.rs`：开局初始化：每队生成 HQ、初始机枪/近防炮。
- `systems.rs`：运行时逻辑（核心战斗循环）：
  - 消费 `ActionEvent` 生成单位（`spawn_units_from_events`）
  - 炮塔行为：机枪旋转射击（`machine_gun_rotate_fire`）、近防炮锁定最近敌人（`ciws_target_fire`）
  - 子弹：边界反射与最低速度（`bullet_move`）、染色地形（`bullet_hit_terrain`）、击中单位/HQ（`bullet_hit_units`）、子弹对撞（`bullet_bullet_collision`）
  - 大球：沿路径占领格子（`bigball_occupy_territory`）、敌方大球碰撞（`bigball_collision`）、边界反弹（`contain_units`）
  - 生命周期：清理耗尽单位并发 `UnitDestroyedEvent`（`cleanup_depleted_units`）
  - 胜负：仅剩一个队伍存活或 HQ 被击中（`check_victory` / `bullet_hit_units` 内写入 `VictoryEvent`）

## 4. 组合方式（“怎么拼起来跑的”）

应用装配见 `src/main.rs`：

1. `App::new()` 创建应用
2. `DefaultPlugins`：窗口、渲染、资产等基础能力
3. `PhysicsPlugins::default()`：引入 Avian2D 物理
4. 插入资源：例如 `Gravity(Vec2::NEG_Y * 490.0)`
5. 注册跨模块消息：`ActionEvent` / `VictoryEvent` / `UnitDestroyedEvent`
6. 加载业务插件：
   - `PinballPlugin`：生成弹珠机关卡 + 每帧处理弹珠碰撞与 UI
   - `TerritoryPlugin`：网格与战斗系统
7. `Startup` 阶段生成 `Camera2d`

从依赖关系看：

- Pinball **只负责产生** `ActionEvent`，不直接依赖 Territory 的内部结构。
- Territory **只消费** `ActionEvent`，不依赖 Pinball 的实现细节。

## 5. 运行时数据模型（ECS 视角）

### 5.1 资源（Resource）

- `Gravity`（物理世界重力）
- `TerritoryGrid`（战场占领网格）
- `Assets<Mesh> / Assets<ColorMaterial> / Assets<Image>` 等渲染资源

### 5.2 实体与组件（Entity + Component）

- 弹珠机侧实体：墙/钉子/加倍区/行动区/出生点/弹珠/弹珠文本
- 战场侧实体：HQ/大球/护盾/机枪/近防炮/子弹/网格渲染精灵（`GridRenderer`）

### 5.3 消息（Message）

用于跨模块通信与物理碰撞：

- `ActionEvent`：Pinball → Territory（行动类型 + 数值 + 队伍）
- `VictoryEvent`：战场胜利广播
- `UnitDestroyedEvent`：单位销毁广播
- `CollisionStart`：物理碰撞开始（由 Avian2D 产生，系统读取）

## 6. 坐标与屏幕布局

本项目把屏幕左右分成两个逻辑区域：

- 左侧弹珠机：`PINBALL_OFFSET_X = -400.0`，宽 `400`，高 `800`（见 `src/pinball/layout.rs`）。
- 右侧战场：逻辑坐标系为 `1024×1024`，渲染区域为 `800×800`，并整体向右偏移 `TERRITORY_RENDER_OFFSET_X = 200.0`（见 `src/territory/coords.rs`）。

战场单位的“物理/渲染位置”主要在渲染坐标中运动，但占领/边界等规则在逻辑坐标/网格坐标中计算，通过 `logic_to_render` / `render_to_logic` 做同步。

## 7. 扩展方式（如何加新功能）

### 7.1 新增一种“行动类型”

1. 在 `src/events.rs` 的 `ActionType` 增加枚举值
2. 在 `src/pinball/layout.rs` 的 `spawn_action_zones` 增加一个对应的 `ActionZoneType` 与显示区
3. 在 `src/pinball/systems.rs` 的 `check_action_zone_collision` 映射到新的 `ActionType`
4. 在 `src/territory/systems.rs` 的 `spawn_units_from_events` 处理该 `ActionType` 并生成单位/触发效果
5. 如需新单位：在 `src/territory/components.rs` 增加组件，并在 `TerritoryPlugin` 的 `Update` 注册相关系统

### 7.2 新增一个系统（System）

优先放在对应模块的 `systems.rs` 中，并在该模块的 `plugin.rs` 里通过 `.add_systems(Startup/Update, ...)` 注册；需要顺序关系时使用 `.after(...)`（项目中 Pinball 的生成顺序已有示例）。

