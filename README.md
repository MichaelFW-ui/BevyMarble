# BevyMarble

BevyMarble 是基于 Rust、Bevy 0.17 和 Avian2D 的双画面 2D 游戏原型。左侧弹珠机产生行动消息，右侧领土战场据此生成单位并进行占领与战斗。

## 运行

在项目根目录执行 `cargo run --release --bin bevymarble`。离线性能分析器的用法和测量范围见[性能分析文档](docs/performance/offline-profiler.md)。

弹珠机编辑器通过 `cargo run --features editor --bin pinball_editor` 启动。支持场地区域、障碍物、出生点与效果的可视化编辑、物理预览，以及多个 profile 的管理。所有配置保存在 `assets/pinball/profiles.json`，游戏自动加载编辑器保存的默认弹珠机。操作说明见[编辑器文档](docs/guides/pinball-editor.md)。

IDE 默认的开发构建也已启用优化：游戏代码使用优化等级 1，Bevy、Avian 等依赖使用等级 3，并保留调试信息。更改配置后的第一次运行需要重新编译依赖，后续运行会使用编译缓存。

## 文档

文档按用途收录在 [docs/README.md](docs/README.md)，包括架构设计、Rust 学习资料、性能记录与项目展示材料。
