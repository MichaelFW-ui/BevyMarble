# 文档目录

文档中的 `src/...`、`assets/...` 路径均以项目根目录为基准。

## 架构与实现

- [设计文档](architecture/design.md)：框架、模块划分、数据流及扩展位置。
- [近防炮数值与冲量](architecture/ciws-defense.md)：弹丸数值、距离衰减配置、染色与命中结算，以及专用冲量和 2M 防御验证。

## 学习资料

- [Rust 语法入门](guides/rust-introduction.md)：结合项目代码介绍 Rust 与 Bevy ECS 中用到的语法。
- [弹珠机编辑器](guides/pinball-editor.md)：布局、效果、物理预览、统一 profile 保存和默认弹珠机选择。

## 性能

- [帧率波动排查与优化](performance/frame-stability.md)：IDE 构建优化、刷新与插值修复、战斗热点、压测前后对比和验证记录。
- [离线性能分析与优化记录](performance/offline-profiler.md)：分析器用法、测量结果、已实施优化和测量边界。
- [早期优化方案](performance/optimization-ideas.md)：历史方案草稿；阅读时以当前代码和性能记录为准。

## 项目展示

- [项目分析与简历素材](career/resume-analysis.md)：技术亮点、简历叙述和面试准备材料。
