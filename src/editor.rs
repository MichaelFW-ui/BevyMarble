//! 独立编辑器入口；由 editor feature 启用。
use avian2d::prelude::*;
use bevy::app::AppExit;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use std::{fs, path::PathBuf};

use crate::colors::TeamColor;
use crate::events::ActionEvent;
use crate::pinball::profile::*;
use crate::pinball::{Marble, PinballPlugin, PinballSimulation, restart_pinball};
use crate::profiler::Profiler;

#[derive(Clone)]
struct Snapshot {
    library: ProfileLibrary,
    profile_index: usize,
    selected: Option<u64>,
}

#[derive(Clone)]
struct CanvasDrag {
    object: PinballObject,
    origin: Vec2,
    resize: bool,
}

#[derive(Resource)]
struct EditorState {
    library: ProfileLibrary,
    saved: ProfileLibrary,
    disk_content: Vec<u8>,
    path: PathBuf,
    profile_index: usize,
    selected: Option<u64>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    gesture: Option<Snapshot>,
    skip_capture: bool,
    drag: Option<CanvasDrag>,
    filter: String,
    zoom: f32,
    pan: egui::Vec2,
    snap: bool,
    playing: bool,
    restart: bool,
    preview_profile: Option<PinballProfile>,
    status: String,
    last_action: String,
    close_requested: bool,
    delete_profile: bool,
    reload_requested: bool,
}

impl EditorState {
    fn load() -> Result<Self, String> {
        let path = profile_path();
        let library = ProfileLibrary::load(&path)?;
        let disk_content = fs::read(&path).map_err(|error| error.to_string())?;
        let profile_index = library
            .profiles
            .iter()
            .position(|profile| profile.id == library.default_profile)
            .unwrap();
        Ok(Self {
            saved: library.clone(),
            library,
            disk_content,
            path,
            profile_index,
            selected: None,
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            skip_capture: false,
            drag: None,
            filter: String::new(),
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            snap: true,
            playing: false,
            restart: false,
            preview_profile: None,
            status: "已载入 profile".into(),
            last_action: String::new(),
            close_requested: false,
            delete_profile: false,
            reload_requested: false,
        })
    }

    fn profile(&self) -> &PinballProfile {
        &self.library.profiles[self.profile_index]
    }
    fn profile_mut(&mut self) -> &mut PinballProfile {
        &mut self.library.profiles[self.profile_index]
    }
    fn dirty(&self) -> bool {
        self.library != self.saved
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            library: self.library.clone(),
            profile_index: self.profile_index,
            selected: self.selected,
        }
    }
    fn restore(&mut self, snapshot: Snapshot) {
        self.library = snapshot.library;
        self.profile_index = snapshot.profile_index;
        self.selected = snapshot.selected;
        self.drag = None;
        self.gesture = None;
        self.skip_capture = true;
        self.restart = true;
    }
    fn record(&mut self, snapshot: Snapshot) {
        self.undo.push(snapshot);
        if self.undo.len() > 128 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn finish_gesture(&mut self) {
        if let Some(snapshot) = self.gesture.take() {
            if snapshot.library != self.library {
                self.record(snapshot);
            }
        }
    }
    fn undo(&mut self) {
        self.finish_gesture();
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(previous);
        }
    }
    fn redo(&mut self) {
        self.finish_gesture();
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(next);
        }
    }
    fn save(&mut self) -> bool {
        let result = (|| {
            let current = fs::read(&self.path).map_err(|error| error.to_string())?;
            if current != self.disk_content {
                return Err("文件已被其他程序修改，请重新载入后保存".into());
            }
            self.library.save(&self.path)?;
            self.disk_content = fs::read(&self.path).map_err(|error| error.to_string())?;
            self.saved = self.library.clone();
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => {
                self.status = "已保存全部 profile".into();
                true
            }
            Err(error) => {
                self.status = error;
                false
            }
        }
    }
    fn reload(&mut self) {
        match ProfileLibrary::load(&self.path) {
            Ok(library) => {
                match fs::read(&self.path) {
                    Ok(content) => self.disk_content = content,
                    Err(error) => {
                        self.status = error.to_string();
                        return;
                    }
                }
                let index = library
                    .profiles
                    .iter()
                    .position(|profile| profile.id == self.profile().id)
                    .or_else(|| {
                        library
                            .profiles
                            .iter()
                            .position(|profile| profile.id == library.default_profile)
                    })
                    .unwrap();
                let previous = self.snapshot();
                self.saved = library.clone();
                self.library = library;
                self.profile_index = index;
                self.selected = None;
                self.record(previous);
                self.skip_capture = true;
                self.restart = true;
                self.status = "已重新载入文件；可撤销恢复之前的编辑".into();
            }
            Err(error) => self.status = error,
        }
    }
    fn unique_profile_id(&self) -> String {
        (1..)
            .map(|index| format!("machine-{index}"))
            .find(|id| {
                !self
                    .library
                    .profiles
                    .iter()
                    .any(|profile| &profile.id == id)
            })
            .unwrap()
    }
    fn add_profile(&mut self, duplicate: bool) {
        let mut profile = if duplicate {
            self.profile().clone()
        } else {
            PinballProfile::default()
        };
        profile.id = self.unique_profile_id();
        profile.name = if duplicate {
            format!("{} Copy", profile.name)
        } else {
            profile.id.clone()
        };
        self.library.profiles.push(profile);
        self.switch_profile(self.library.profiles.len() - 1);
    }
    fn switch_profile(&mut self, index: usize) {
        self.profile_index = index;
        self.selected = None;
        self.drag = None;
        self.pan = egui::Vec2::ZERO;
        self.zoom = 1.0;
        self.restart = true;
        self.last_action.clear();
    }
    fn remove_profile(&mut self) {
        if self.library.profiles.len() <= 1 {
            return;
        }
        let removed = self.library.profiles.remove(self.profile_index);
        self.profile_index = self.profile_index.min(self.library.profiles.len() - 1);
        if self.library.default_profile == removed.id {
            self.library.default_profile = self.profile().id.clone();
        }
        self.switch_profile(self.profile_index);
    }
    fn add_object(&mut self, kind: ObjectKind) {
        let size = match kind {
            ObjectKind::Wall => [100.0, 10.0],
            ObjectKind::Peg => [16.0; 2],
            ObjectKind::Spawn { .. } => [12.0; 2],
            _ => [80.0, 40.0],
        };
        let id = self.profile().next_object_id();
        self.profile_mut()
            .objects
            .push(PinballObject::new(id, kind, [0.0; 2], size));
        self.selected = Some(id);
    }
    fn duplicate_object(&mut self) {
        if let Some(mut object) = self
            .profile()
            .objects
            .iter()
            .find(|object| Some(object.id) == self.selected)
            .cloned()
        {
            object.id = self.profile().next_object_id();
            object.name = format!("{} Copy", object.name);
            object.position[0] += 10.0;
            object.position[1] -= 10.0;
            self.selected = Some(object.id);
            self.profile_mut().objects.push(object);
        }
    }
    fn delete_object(&mut self) {
        let selected = self.selected.take();
        self.profile_mut()
            .objects
            .retain(|object| Some(object.id) != selected);
    }
}

pub fn run() -> Result<(), String> {
    let state = EditorState::load()?;
    let profile = state.profile().clone();
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "弹珠机编辑器".into(),
                        resolution: (1440, 960).into(),
                        ..default()
                    }),
                    close_when_requested: false,
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins(PhysicsPlugins::default())
        .insert_resource(Gravity(Vec2::from_array(profile.gravity)))
        .insert_resource(profile)
        .insert_resource(state)
        .insert_resource(PinballSimulation(false))
        .init_resource::<Profiler>()
        .add_message::<ActionEvent>()
        .add_plugins(PinballPlugin)
        .add_systems(Startup, setup_editor)
        .add_systems(First, sync_preview)
        .add_systems(Update, (handle_close, read_preview_actions))
        .add_systems(EguiPrimaryContextPass, editor_ui)
        .run();
    Ok(())
}

fn setup_editor(mut commands: Commands) {
    // 物理预览由 egui 画布绘制；相机只用于承载编辑器 UI。
    commands.spawn((Camera2d, RenderLayers::layer(31)));
}

fn configure_editor_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in [
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ] {
        if let Ok(bytes) = fs::read(path) {
            fonts
                .font_data
                .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .insert(0, "cjk".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
}

fn sync_preview(world: &mut World) {
    let (playing, restart, profile) = {
        let mut state = world.resource_mut::<EditorState>();
        let profile = state.profile().clone();
        if profile.validate().is_err() {
            state.playing = false;
        }
        let restart = state.restart || state.preview_profile.as_ref() != Some(&profile);
        state.restart = false;
        (state.playing, restart, profile)
    };
    world.resource_mut::<PinballSimulation>().0 = playing;
    if playing {
        if restart {
            world.insert_resource(profile.clone());
            world.insert_resource(Gravity(Vec2::from_array(profile.gravity)));
            restart_pinball(world);
            world.resource_mut::<EditorState>().preview_profile = Some(profile);
        }
        world.resource_mut::<Time<Physics>>().unpause();
    } else {
        world.resource_mut::<Time<Physics>>().pause();
        // 下次开始预览时使用最新布局。
        if restart {
            world.resource_mut::<EditorState>().preview_profile = None;
        }
    }
}

fn handle_close(
    mut events: MessageReader<WindowCloseRequested>,
    mut state: ResMut<EditorState>,
    mut exit: MessageWriter<AppExit>,
) {
    if events.read().next().is_some() {
        if state.dirty() {
            state.close_requested = true;
        } else {
            exit.write(AppExit::Success);
        }
    }
}

fn read_preview_actions(mut events: MessageReader<ActionEvent>, mut state: ResMut<EditorState>) {
    for event in events.read() {
        state.last_action = format!(
            "{:?} → {:?}  {}",
            event.team,
            event.action_type,
            crate::pinball::format_value(event.value)
        );
    }
}

fn editor_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<EditorState>,
    marbles: Query<(&Marble, &Transform)>,
    mut exit: MessageWriter<AppExit>,
    mut fonts_ready: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*fonts_ready {
        configure_editor_fonts(ctx);
        *fonts_ready = true;
    }
    let before = state.snapshot();
    state.skip_capture = false;
    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
        state.save();
    }
    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
        state.undo();
    }
    if ctx.input_mut(|input| {
        input.consume_key(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        )
    }) {
        state.redo();
    }
    if !ctx.wants_keyboard_input() && !state.playing {
        if ctx.input(|input| {
            input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
        }) {
            state.delete_object();
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            state.duplicate_object();
        }
        let step = if ctx.input(|input| input.modifiers.shift) {
            10.0
        } else {
            1.0
        };
        let movement = ctx.input(|input| {
            Vec2::new(
                f32::from(input.key_pressed(egui::Key::ArrowRight))
                    - f32::from(input.key_pressed(egui::Key::ArrowLeft)),
                f32::from(input.key_pressed(egui::Key::ArrowUp))
                    - f32::from(input.key_pressed(egui::Key::ArrowDown)),
            )
        }) * step;
        let selected = state.selected;
        if movement != Vec2::ZERO {
            if let Some(object) = state
                .profile_mut()
                .objects
                .iter_mut()
                .find(|object| Some(object.id) == selected)
            {
                object.position = (Vec2::from_array(object.position) + movement).to_array();
            }
        }
    }
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.heading("弹珠机编辑器");
            ui.separator();
            let mut index = state.profile_index;
            egui::ComboBox::from_id_salt("profile")
                .selected_text(&state.profile().name)
                .show_ui(ui, |ui| {
                    for (i, profile) in state.library.profiles.iter().enumerate() {
                        let label = if profile.id == state.library.default_profile {
                            format!("{} · 默认", profile.name)
                        } else {
                            profile.name.clone()
                        };
                        ui.selectable_value(&mut index, i, label);
                    }
                });
            if index != state.profile_index {
                state.finish_gesture();
                state.switch_profile(index);
            }
            if ui.button("新建").clicked() {
                state.add_profile(false);
            }
            if ui.button("复制弹珠机").clicked() {
                state.add_profile(true);
            }
            if ui
                .add_enabled(
                    state.library.profiles.len() > 1,
                    egui::Button::new("删除弹珠机"),
                )
                .clicked()
            {
                state.delete_profile = true;
            }
            if ui.button("设为游戏默认").clicked() {
                state.library.default_profile = state.profile().id.clone();
            }
            ui.separator();
            if ui.button("保存全部").clicked() {
                state.save();
            }
            if ui.button("重新载入").clicked() {
                if state.dirty() {
                    state.reload_requested = true;
                } else {
                    state.reload();
                }
            }
            if ui
                .add_enabled(
                    !state.undo.is_empty() || state.gesture.is_some(),
                    egui::Button::new("撤销"),
                )
                .clicked()
            {
                state.undo();
            }
            if ui
                .add_enabled(!state.redo.is_empty(), egui::Button::new("重做"))
                .clicked()
            {
                state.redo();
            }
            if state.dirty() {
                ui.colored_label(egui::Color32::YELLOW, "● 未保存");
            }
        });
    });
    egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
        if let Err(error) = state.library.validate() {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(&state.status);
            ui.separator();
            ui.label(format!("游戏默认：{}", state.library.active().name));
            ui.label(state.path.display().to_string());
        });
    });
    egui::SidePanel::left("objects")
        .default_width(235.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("场景对象");
            ui.text_edit_singleline(&mut state.filter);
            ui.menu_button("添加对象", |ui| {
                for (label, kind) in object_kinds() {
                    if ui.button(label).clicked() {
                        state.add_object(kind);
                        ui.close();
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(state.selected.is_some(), egui::Button::new("复制"))
                    .clicked()
                {
                    state.duplicate_object();
                }
                if ui
                    .add_enabled(state.selected.is_some(), egui::Button::new("删除"))
                    .clicked()
                {
                    state.delete_object();
                }
            });
            ui.separator();
            if ui
                .selectable_label(state.selected.is_none(), "场地与弹珠设置")
                .clicked()
            {
                state.selected = None;
            }
            let filter = state.filter.to_lowercase();
            let selected = state.selected;
            let mut selection = selected;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for object in &mut state.profile_mut().objects {
                    if !object.name.to_lowercase().contains(&filter) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut object.enabled, "");
                        if ui
                            .selectable_label(selected == Some(object.id), &object.name)
                            .clicked()
                        {
                            selection = Some(object.id);
                        }
                    });
                }
            });
            state.selected = selection;
        });
    egui::SidePanel::right("inspector")
        .default_width(290.0)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| inspector(ui, &mut state));
        });
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(if state.playing {
                    "停止预览"
                } else {
                    "运行预览"
                })
                .clicked()
            {
                if let Err(error) = state.profile().validate() {
                    state.status = error;
                } else {
                    state.playing = !state.playing;
                }
            }
            if ui.button("重置弹珠").clicked() {
                state.restart = true;
            }
            if ui.button("适应画布").clicked() {
                state.zoom = 1.0;
                state.pan = egui::Vec2::ZERO;
            }
            ui.checkbox(&mut state.snap, "吸附网格");
            ui.add(egui::Slider::new(&mut state.zoom, 0.25..=4.0).text("视图缩放"));
        });
        ui.label("单击选择 · 拖动移动 · 右下角手柄缩放 · 中键平移 · 滚轮缩放 · 方向键微调");
        if !state.last_action.is_empty() {
            ui.label(&state.last_action);
        }
        canvas(ui, &mut state, &marbles);
    });
    if state.delete_profile {
        egui::Window::new("删除弹珠机")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("删除 {}？可以通过撤销恢复。", state.profile().name));
                ui.horizontal(|ui| {
                    if ui.button("删除").clicked() {
                        state.remove_profile();
                        state.delete_profile = false;
                    }
                    if ui.button("取消").clicked() {
                        state.delete_profile = false;
                    }
                });
            });
    }
    if state.reload_requested {
        egui::Window::new("重新载入 profile")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("当前编辑尚未保存。重新载入后，可以通过撤销恢复这些编辑。");
                ui.horizontal(|ui| {
                    if ui.button("重新载入").clicked() {
                        state.reload();
                        state.reload_requested = false;
                    }
                    if ui.button("取消").clicked() {
                        state.reload_requested = false;
                    }
                });
            });
    }
    if state.close_requested {
        egui::Window::new("保存编辑")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("当前 profile 有未保存的修改。");
                ui.horizontal(|ui| {
                    if ui.button("保存并退出").clicked() && state.save() {
                        exit.write(AppExit::Success);
                    }
                    if ui.button("放弃修改并退出").clicked() {
                        exit.write(AppExit::Success);
                    }
                    if ui.button("继续编辑").clicked() {
                        state.close_requested = false;
                    }
                });
            });
    }
    if !state.skip_capture && before.library != state.library {
        if ctx.input(|input| input.pointer.primary_down()) {
            if state.gesture.is_none() {
                state.gesture = Some(before);
            }
        } else {
            if state.gesture.is_none() {
                state.record(before);
            }
        }
    }
    if !ctx.input(|input| input.pointer.primary_down()) {
        state.finish_gesture();
    }
    Ok(())
}

fn object_kinds() -> Vec<(&'static str, ObjectKind)> {
    vec![
        ("墙 / 挡板", ObjectKind::Wall),
        ("钉子", ObjectKind::Peg),
        (
            "加倍区域",
            ObjectKind::Multiplier {
                factor: 2,
                reset_position: true,
            },
        ),
        (
            "行动区域",
            ObjectKind::Action {
                action: crate::pinball::ActionZoneType::BigBall,
                value_scale: 1.0,
                reset_position: true,
            },
        ),
        (
            "加速区域",
            ObjectKind::Boost {
                velocity: [0.0, 300.0],
            },
        ),
        (
            "出生点",
            ObjectKind::Spawn {
                team: TeamColor::Red,
            },
        ),
    ]
}

fn drag_value(value: &mut impl egui::emath::Numeric) -> egui::DragValue<'_> {
    egui::DragValue::new(value).clamp_existing_to_range(false)
}

fn pair(
    ui: &mut egui::Ui,
    label: &str,
    values: &mut [f32; 2],
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.add(
            drag_value(&mut values[0])
                .range(range.clone())
                .speed(speed)
                .prefix("X "),
        );
        ui.add(
            drag_value(&mut values[1])
                .range(range)
                .speed(speed)
                .prefix("Y "),
        );
    });
}

fn inspector(ui: &mut egui::Ui, state: &mut EditorState) {
    let selected = state.selected;
    if let Some(object) = state
        .profile_mut()
        .objects
        .iter_mut()
        .find(|object| Some(object.id) == selected)
    {
        ui.heading("对象属性");
        ui.label(format!("ID {}", object.id));
        ui.text_edit_singleline(&mut object.name);
        ui.checkbox(&mut object.enabled, "启用");
        let mut kind_index = match object.kind {
            ObjectKind::Wall => 0,
            ObjectKind::Peg => 1,
            ObjectKind::Multiplier { .. } => 2,
            ObjectKind::Action { .. } => 3,
            ObjectKind::Boost { .. } => 4,
            ObjectKind::Spawn { .. } => 5,
        };
        let previous = kind_index;
        let kinds = object_kinds();
        egui::ComboBox::from_id_salt("kind")
            .selected_text(kinds[kind_index].0)
            .show_ui(ui, |ui| {
                for (index, (label, _)) in kinds.iter().enumerate() {
                    ui.selectable_value(&mut kind_index, index, *label);
                }
            });
        if kind_index != previous {
            object.kind = kinds[kind_index].1.clone();
        }
        ui.separator();
        pair(ui, "位置", &mut object.position, -20000.0..=20000.0, 1.0);
        pair(
            ui,
            "基础宽高（椭圆为直径）",
            &mut object.size,
            0.1..=10000.0,
            1.0,
        );
        pair(ui, "独立缩放", &mut object.scale, 0.01..=100.0, 0.01);
        ui.label(format!(
            "实际尺寸 {:.1} × {:.1}",
            object.dimensions().x,
            object.dimensions().y
        ));
        ui.horizontal(|ui| {
            ui.label("旋转角度");
            ui.add(
                drag_value(&mut object.rotation)
                    .speed(1.0)
                    .range(-360.0..=360.0)
                    .suffix("°"),
            );
        });
        egui::ComboBox::from_id_salt("shape")
            .selected_text(match object.shape {
                ObjectShape::Rectangle => "矩形",
                ObjectShape::Ellipse => "椭圆 / 圆",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut object.shape, ObjectShape::Rectangle, "矩形");
                ui.selectable_value(&mut object.shape, ObjectShape::Ellipse, "椭圆 / 圆");
            });
        ui.label("颜色与透明度");
        let mut edited_color = object.color.map(|channel| (channel * 255.0).round() as u8);
        if ui
            .color_edit_button_srgba_unmultiplied(&mut edited_color)
            .changed()
        {
            object.color = edited_color.map(|channel| channel as f32 / 255.0);
        }
        ui.separator();
        ui.heading("效果");
        match &mut object.kind {
            ObjectKind::Wall | ObjectKind::Peg => {
                ui.add(
                    egui::Slider::new(&mut object.restitution, 0.0..=1.0)
                        .clamping(egui::SliderClamping::Edits)
                        .min_decimals(2)
                        .max_decimals(4)
                        .text("碰撞弹性"),
                );
                ui.add(
                    egui::Slider::new(&mut object.friction, 0.0..=10.0)
                        .clamping(egui::SliderClamping::Edits)
                        .min_decimals(2)
                        .max_decimals(4)
                        .text("摩擦"),
                );
            }
            ObjectKind::Multiplier {
                factor,
                reset_position,
            } => {
                ui.horizontal(|ui| {
                    ui.label("加倍倍率");
                    ui.add(drag_value(factor).range(1..=32_000_000_000_u64));
                });
                ui.checkbox(reset_position, "触发后回到出生点");
            }
            ObjectKind::Action {
                action,
                value_scale,
                reset_position,
            } => {
                use crate::pinball::ActionZoneType;
                egui::ComboBox::from_id_salt("action")
                    .selected_text(format!("{action:?}"))
                    .show_ui(ui, |ui| {
                        for (label, value) in [
                            ("大球", ActionZoneType::BigBall),
                            ("护盾", ActionZoneType::Shield),
                            ("机枪", ActionZoneType::MachineGun),
                            ("近防炮", ActionZoneType::CIWS),
                        ] {
                            ui.selectable_value(action, value, label);
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("行动数值系数");
                    ui.add(drag_value(value_scale).speed(0.1).range(0.01..=1000.0));
                });
                ui.checkbox(reset_position, "触发后回到出生点");
                ui.label("触发行动后，弹珠数值恢复为初始值。");
            }
            ObjectKind::Boost { velocity } => {
                pair(ui, "触发时增加速度", velocity, -10000.0..=10000.0, 5.0)
            }
            ObjectKind::Spawn { team } => {
                egui::ComboBox::from_id_salt("team")
                    .selected_text(format!("{team:?}"))
                    .show_ui(ui, |ui| {
                        for value in TeamColor::all() {
                            ui.selectable_value(team, value, format!("{value:?}"));
                        }
                    });
                ui.label("每个队伍保留一个启用的出生点。");
                ui.label("四队出生点共同定义随机投放范围。");
            }
        }
    } else {
        ui.heading("场地与弹珠");
        let profile = state.profile_mut();
        ui.label("弹珠机名称");
        ui.text_edit_singleline(&mut profile.name);
        ui.label(format!("ID {}", profile.id));
        ui.horizontal(|ui| {
            ui.label("场地宽度");
            ui.add(drag_value(&mut profile.width).range(100.0..=10000.0));
        });
        ui.horizontal(|ui| {
            ui.label("场地高度");
            ui.add(drag_value(&mut profile.height).range(100.0..=10000.0));
        });
        pair(ui, "重力", &mut profile.gravity, -10000.0..=10000.0, 5.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("初始数值");
            ui.add(drag_value(&mut profile.marble.initial_value).range(1..=32_000_000_000_u64));
        });
        ui.add(
            egui::Slider::new(&mut profile.marble.restitution, 0.0..=1.0)
                .clamping(egui::SliderClamping::Edits)
                .min_decimals(2)
                .max_decimals(4)
                .text("弹珠弹性"),
        );
        ui.add(
            egui::Slider::new(&mut profile.marble.friction, 0.0..=10.0)
                .clamping(egui::SliderClamping::Edits)
                .min_decimals(2)
                .max_decimals(4)
                .text("弹珠摩擦"),
        );
        ui.horizontal(|ui| {
            ui.label("随机水平速度");
            ui.add(drag_value(&mut profile.marble.spawn_speed).range(0.0..=10000.0));
        });
        ui.horizontal(|ui| {
            ui.label("防卡住升力速度");
            ui.add(drag_value(&mut profile.marble.rescue_speed).range(0.0..=10000.0));
        });
        ui.label("场地尺寸定义相机范围和越界回收范围。边界墙可在场景对象中独立调整。");
    }
}

fn color(rgba: [f32; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (rgba[0] * 255.0) as u8,
        (rgba[1] * 255.0) as u8,
        (rgba[2] * 255.0) as u8,
        (rgba[3] * 255.0) as u8,
    )
}

fn canvas(ui: &mut egui::Ui, state: &mut EditorState, marbles: &Query<(&Marble, &Transform)>) {
    let (area, painter) = ui.allocate_painter(
        ui.available_size().max(egui::vec2(100.0, 100.0)),
        egui::Sense::hover(),
    );
    let response = ui.interact(
        area.rect,
        egui::Id::new("pinball-canvas"),
        egui::Sense::click_and_drag(),
    );
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 23, 32));
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0 {
            state.zoom = (state.zoom * (scroll * 0.002).exp()).clamp(0.25, 4.0);
        }
    }
    if response.dragged_by(egui::PointerButton::Middle) {
        state.pan += ui.input(|input| input.pointer.delta());
    }
    let scale = ((rect.width() - 60.0) / state.profile().width)
        .min((rect.height() - 60.0) / state.profile().height)
        .max(0.001)
        * state.zoom;
    let center = rect.center() + state.pan;
    let to_screen = |point: Vec2| center + egui::vec2(point.x * scale, -point.y * scale);
    let to_world =
        |point: egui::Pos2| Vec2::new((point.x - center.x) / scale, (center.y - point.y) / scale);
    let bounds = egui::Rect::from_two_pos(
        to_screen(Vec2::new(
            -state.profile().width / 2.0,
            state.profile().height / 2.0,
        )),
        to_screen(Vec2::new(
            state.profile().width / 2.0,
            -state.profile().height / 2.0,
        )),
    );
    painter.rect_filled(bounds, 0.0, egui::Color32::from_rgb(25, 32, 43));
    // 屏幕空间计算可见网格，控制大场地和高缩放下的绘制量。
    let grid_step = if scale < 0.3 { 100.0 } else { 20.0 };
    let min = to_world(rect.left_bottom());
    let max = to_world(rect.right_top());
    for index in (min.x / grid_step).floor() as i32..=(max.x / grid_step).ceil() as i32 {
        painter.line_segment(
            [
                to_screen(Vec2::new(index as f32 * grid_step, min.y)),
                to_screen(Vec2::new(index as f32 * grid_step, max.y)),
            ],
            egui::Stroke::new(0.5, egui::Color32::from_gray(45)),
        );
    }
    for index in (min.y / grid_step).floor() as i32..=(max.y / grid_step).ceil() as i32 {
        painter.line_segment(
            [
                to_screen(Vec2::new(min.x, index as f32 * grid_step)),
                to_screen(Vec2::new(max.x, index as f32 * grid_step)),
            ],
            egui::Stroke::new(0.5, egui::Color32::from_gray(45)),
        );
    }
    painter.rect_stroke(
        bounds,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(95)),
        egui::StrokeKind::Inside,
    );
    let handle_position = |object: &PinballObject| {
        to_screen(
            Vec2::from_array(object.position)
                + Mat2::from_angle(object.rotation.to_radians())
                    * (object.dimensions() * Vec2::new(0.5, -0.5)),
        )
    };
    if !state.playing {
        if response.clicked_by(egui::PointerButton::Primary)
            || response.drag_started_by(egui::PointerButton::Primary)
        {
            if let Some(pointer) = ui
                .input(|input| input.pointer.press_origin())
                .or(response.interact_pointer_pos())
            {
                let world = to_world(pointer);
                let resize = state
                    .profile()
                    .objects
                    .iter()
                    .find(|object| Some(object.id) == state.selected)
                    .is_some_and(|object| handle_position(object).distance(pointer) < 12.0);
                if !resize {
                    state.selected = state
                        .profile()
                        .objects
                        .iter()
                        .rev()
                        .find(|object| object.contains(world))
                        .map(|object| object.id);
                }
                if response.drag_started_by(egui::PointerButton::Primary) {
                    state.drag = state
                        .profile()
                        .objects
                        .iter()
                        .find(|object| Some(object.id) == state.selected)
                        .map(|object| CanvasDrag {
                            object: object.clone(),
                            origin: world,
                            resize,
                        });
                }
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(drag), Some(pointer)) =
                (state.drag.clone(), response.interact_pointer_pos())
            {
                let delta = to_world(pointer) - drag.origin;
                let snap = state.snap;
                if let Some(object) = state
                    .profile_mut()
                    .objects
                    .iter_mut()
                    .find(|object| object.id == drag.object.id)
                {
                    if drag.resize {
                        let local = Mat2::from_angle(-object.rotation.to_radians()) * delta;
                        let dimensions = (drag.object.dimensions() + local * Vec2::new(2.0, -2.0))
                            .max(Vec2::splat(0.1));
                        object.scale = (dimensions / Vec2::from_array(object.size))
                            .clamp(Vec2::splat(0.01), Vec2::splat(100.0))
                            .to_array();
                    } else {
                        let mut position = Vec2::from_array(drag.object.position) + delta;
                        if snap {
                            position = (position / 10.0).round() * 10.0;
                        }
                        object.position = position.to_array();
                    }
                }
            }
        }
        if response.drag_stopped() {
            state.drag = None;
        }
    }
    for object in &state.profile().objects {
        let rotation = Mat2::from_angle(object.rotation.to_radians());
        let half = object.dimensions() / 2.0;
        let positions = match object.shape {
            ObjectShape::Rectangle => vec![
                Vec2::new(-half.x, -half.y),
                Vec2::new(half.x, -half.y),
                Vec2::new(half.x, half.y),
                Vec2::new(-half.x, half.y),
            ],
            ObjectShape::Ellipse => (0..48)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / 48.0;
                    Vec2::new(angle.cos() * half.x, angle.sin() * half.y)
                })
                .collect(),
        };
        let points = positions
            .into_iter()
            .map(|point| to_screen(Vec2::from_array(object.position) + rotation * point))
            .collect();
        let selected = Some(object.id) == state.selected;
        let fill = if object.enabled {
            color(object.color)
        } else {
            egui::Color32::from_rgba_unmultiplied(90, 90, 90, 45)
        };
        let stroke = egui::Stroke::new(
            if selected { 2.0 } else { 0.5 },
            if selected {
                egui::Color32::from_rgb(100, 200, 255)
            } else {
                egui::Color32::from_gray(140)
            },
        );
        painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        if !object.kind.is_solid() || selected {
            let label = match object.kind {
                ObjectKind::Spawn { team } => match team {
                    TeamColor::Red => "红",
                    TeamColor::Blue => "蓝",
                    TeamColor::Green => "绿",
                    TeamColor::Yellow => "黄",
                }
                .into(),
                _ => object.kind.label(),
            };
            painter.text(
                to_screen(Vec2::from_array(object.position)),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional((14.0 * scale).clamp(9.0, 20.0)),
                egui::Color32::WHITE,
            );
        }
        if selected && !state.playing {
            painter.rect_filled(
                egui::Rect::from_center_size(handle_position(object), egui::vec2(10.0, 10.0)),
                1.0,
                egui::Color32::from_rgb(100, 200, 255),
            );
        }
    }
    if state.playing {
        for (marble, transform) in marbles.iter() {
            let position = to_screen(transform.translation.truncate());
            let radius = crate::pinball::calculate_radius(marble.value) * scale;
            painter.circle_filled(
                position,
                radius,
                color(marble.team.to_color().to_srgba().to_f32_array()),
            );
            painter.text(
                position,
                egui::Align2::CENTER_CENTER,
                crate::pinball::format_value(marble.value),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
        }
    }
    if let Some(pointer) = response.hover_pos() {
        let position = to_world(pointer);
        painter.text(
            rect.left_bottom() + egui::vec2(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            format!("X {:.1}  Y {:.1}", position.x, position.y),
            egui::FontId::monospace(12.0),
            egui::Color32::GRAY,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspecting_profiles_does_not_modify_saved_values() {
        let mut state = EditorState::load().unwrap();
        let original = state.library.clone();
        let ctx = egui::Context::default();
        for profile_index in 0..state.library.profiles.len() {
            state.switch_profile(profile_index);
            let ids: Vec<_> = state
                .profile()
                .objects
                .iter()
                .map(|object| object.id)
                .collect();
            for selected in std::iter::once(None).chain(ids.into_iter().map(Some)) {
                state.selected = selected;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| inspector(ui, &mut state));
                });
            }
        }
        assert_eq!(state.library, original);
        assert!(!state.dirty());
    }

    #[test]
    fn profile_switch_preserves_edits_and_delete_default_is_undoable() {
        let mut state = EditorState::load().unwrap();
        let before = state.snapshot();
        state.profile_mut().objects[0].scale = [2.0, 3.0];
        state.add_profile(true);
        let new_id = state.profile().id.clone();
        state.library.default_profile = new_id.clone();
        state.switch_profile(0);
        assert_eq!(state.profile().objects[0].scale, [2.0, 3.0]);
        state.switch_profile(state.library.profiles.len() - 1);
        state.remove_profile();
        assert_ne!(state.library.default_profile, new_id);
        state.library.validate().unwrap();
        state.record(before.clone());
        state.undo();
        assert_eq!(state.library, before.library);
        state.redo();
        assert_eq!(state.library.profiles[0].objects[0].scale, [2.0, 3.0]);
    }

    #[test]
    fn save_checks_external_changes_and_rejects_invalid_profiles() {
        let mut state = EditorState::load().unwrap();
        state.path = std::env::temp_dir().join(format!(
            "bevymarble-editor-test-{}.json",
            std::process::id()
        ));
        fs::write(&state.path, &state.disk_content).unwrap();
        state.profile_mut().name = "Edited".into();
        assert!(state.save());
        assert!(!state.dirty());
        let saved_content = fs::read(&state.path).unwrap();
        state.profile_mut().objects[0].size[0] = 0.0;
        assert!(!state.save());
        assert_eq!(fs::read(&state.path).unwrap(), saved_content);
        state.library = state.saved.clone();
        fs::write(&state.path, b"external change").unwrap();
        assert!(!state.save());
        assert_eq!(fs::read(&state.path).unwrap(), b"external change");
        fs::remove_file(&state.path).unwrap();
    }

    #[test]
    fn canvas_gesture_is_one_undo_step_and_new_edit_clears_redo() {
        let mut state = EditorState::load().unwrap();
        let original = state.snapshot();
        state.gesture = Some(original.clone());
        state.profile_mut().objects[0].position[0] += 10.0;
        state.profile_mut().objects[0].position[0] += 20.0;
        state.finish_gesture();
        assert_eq!(state.undo.len(), 1);
        state.undo();
        assert_eq!(state.library, original.library);
        let before = state.snapshot();
        state.add_object(ObjectKind::Peg);
        state.record(before);
        assert!(state.redo.is_empty());
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use bevy::camera::RenderTarget;
    use bevy::render::render_resource::TextureFormat;
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Captured(Option<Image>);

    fn input(app: &mut App, camera: Entity, events: impl IntoIterator<Item = egui::Event>) {
        for event in events {
            app.world_mut()
                .write_message(bevy_egui::input::EguiInputEvent {
                    context: camera,
                    event,
                });
        }
        app.update();
    }

    fn pointer(app: &mut App, camera: Entity, position: egui::Pos2, pressed: bool) {
        input(
            app,
            camera,
            [
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }

    fn shortcut(app: &mut App, camera: Entity, key: egui::Key) {
        for pressed in [true, false] {
            input(
                app,
                camera,
                [egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
            );
        }
    }

    fn capture(app: &mut App, target: &Handle<Image>, name: &str) -> Image {
        for _ in 0..45 {
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |event: On<ScreenshotCaptured>, mut image: ResMut<Captured>| {
                    image.0 = Some(event.image.clone());
                },
            );
        for _ in 0..100 {
            app.update();
            if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
                if let Some(directory) = std::env::var_os("BEVY_LAYOUT_PREVIEW_DIR") {
                    image
                        .clone()
                        .try_into_dynamic()
                        .unwrap()
                        .save(PathBuf::from(directory).join(format!("{name}.png")))
                        .unwrap();
                }
                return image;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("GPU 未返回编辑器画面");
    }

    #[test]
    #[ignore = "requires a GPU adapter; renders the editor without opening a window"]
    fn editor_renders_profiles_and_runs_shared_physics() {
        let mut app = App::new();
        let state = EditorState::load().unwrap();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
        )
        .add_plugins((EguiPlugin::default(), PhysicsPlugins::default()))
        // 离屏相机没有窗口事件循环；手动维护上下文尺寸。
        .configure_sets(
            PreUpdate,
            bevy_egui::EguiPreUpdateSet::InitContexts.run_if(|| false),
        )
        .add_systems(
            PreUpdate,
            bevy_egui::update_ui_size_and_scale_system
                .before(bevy_egui::EguiPreUpdateSet::ProcessInput),
        )
        .insert_resource(state.profile().clone())
        .insert_resource(Gravity(Vec2::NEG_Y * 490.0))
        .insert_resource(state)
        .insert_resource(PinballSimulation(false))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .init_resource::<Profiler>()
        .init_resource::<Captured>()
        .add_message::<ActionEvent>()
        .add_plugins(PinballPlugin)
        .add_systems(First, sync_preview)
        .add_systems(Update, read_preview_actions)
        .add_systems(EguiPrimaryContextPass, editor_ui);
        let target =
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::new_target_texture(
                    1440,
                    960,
                    TextureFormat::Rgba8UnormSrgb,
                ));
        let camera = app
            .world_mut()
            .spawn((
                Camera2d,
                bevy_egui::PrimaryEguiContext,
                RenderLayers::layer(31),
                Camera {
                    target: RenderTarget::Image(target.clone().into()),
                    ..default()
                },
            ))
            .id();
        app.finish();
        app.cleanup();
        let classic = capture(&mut app, &target, "editor-classic");
        let pixels = classic.clone().try_into_dynamic().unwrap().to_rgba8();
        assert!(
            pixels
                .pixels()
                .filter(|pixel| pixel[1] > pixel[0].saturating_add(25))
                .count()
                > 500,
            "加倍区域未渲染"
        );
        // 使用真实 egui 输入验证画布移动、缩放与撤销。
        let canvas_rect = app
            .world_mut()
            .get_mut::<bevy_egui::EguiContext>(camera)
            .unwrap()
            .get_mut()
            .read_response(egui::Id::new("pinball-canvas"))
            .unwrap()
            .rect;
        let scale =
            ((canvas_rect.width() - 60.0) / 400.0).min((canvas_rect.height() - 60.0) / 800.0);
        let position =
            |world: Vec2| canvas_rect.center() + egui::vec2(world.x * scale, -world.y * scale);
        let original = app.world().resource::<EditorState>().profile().objects[5].clone();
        pointer(
            &mut app,
            camera,
            position(Vec2::from_array(original.position)),
            true,
        );
        let moved = Vec2::from_array(original.position) + Vec2::new(30.0, -20.0);
        input(
            &mut app,
            camera,
            [egui::Event::PointerMoved(position(moved))],
        );
        pointer(&mut app, camera, position(moved), false);
        assert_eq!(
            app.world().resource::<EditorState>().selected,
            Some(original.id)
        );
        assert_eq!(
            app.world().resource::<EditorState>().profile().objects[5].position,
            moved.to_array()
        );
        let handle = moved + Vec2::new(8.0, -8.0);
        pointer(&mut app, camera, position(handle), true);
        input(
            &mut app,
            camera,
            [egui::Event::PointerMoved(position(
                handle + Vec2::new(8.0, -8.0),
            ))],
        );
        pointer(
            &mut app,
            camera,
            position(handle + Vec2::new(8.0, -8.0)),
            false,
        );
        assert!(
            app.world().resource::<EditorState>().profile().objects[5]
                .scale
                .into_iter()
                .all(|value| (value - 2.0).abs() < 0.0001)
        );
        shortcut(&mut app, camera, egui::Key::Z);
        assert_eq!(
            app.world().resource::<EditorState>().profile().objects[5].scale,
            [1.0; 2]
        );
        shortcut(&mut app, camera, egui::Key::Z);
        assert_eq!(
            app.world().resource::<EditorState>().profile().objects[5],
            original
        );
        // 保存快捷键只写测试副本。
        let save_path =
            std::env::temp_dir().join(format!("bevymarble-ui-save-{}.json", std::process::id()));
        {
            let mut state = app.world_mut().resource_mut::<EditorState>();
            fs::write(&save_path, &state.disk_content).unwrap();
            state.path = save_path.clone();
            state.profile_mut().name = "UI saved".into();
        }
        shortcut(&mut app, camera, egui::Key::S);
        assert_eq!(
            ProfileLibrary::load(&save_path).unwrap().profiles[0].name,
            "UI saved"
        );
        assert!(!app.world().resource::<EditorState>().dirty());
        fs::remove_file(save_path).unwrap();
        app.world_mut().resource_mut::<EditorState>().playing = true;
        capture(&mut app, &target, "editor-playing");
        let mut marbles = app.world_mut().query::<(&Marble, &Transform)>();
        assert_eq!(marbles.iter(app.world()).count(), 4);
        assert!(
            marbles
                .iter(app.world())
                .any(|(_, transform)| transform.translation.y < 340.0),
            "弹珠预览未推进物理"
        );
        {
            let mut state = app.world_mut().resource_mut::<EditorState>();
            state.playing = false;
            state.switch_profile(1);
            state.selected = Some(56);
        }
        let zigzag = capture(&mut app, &target, "editor-zigzag");
        assert_ne!(classic.data, zigzag.data);
        assert!(
            !app.world().resource::<EditorState>().dirty(),
            "查看布局和预览不能修改配置"
        );
    }
}
