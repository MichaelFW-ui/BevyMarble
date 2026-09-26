use bevy::prelude::*;
use std::collections::VecDeque;
use bevy::image::ImageSampler;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;
use bevy::camera::visibility::RenderLayers;

use crate::profiler::{CounterId, Profiler, ScopeId};
use super::grid::{TerritoryGrid, TOTAL_TILES};
use super::coords::{TERRITORY_LOGIC_HEIGHT, TERRITORY_LOGIC_WIDTH};

/// 网格渲染组件
#[derive(Component)]
pub struct GridRenderer;

/// GPU 网格材质
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GridMaterial {
    // 使用整数纹理避免每次更新都重建 GPU buffer（ShaderStorageBuffer 资产会在 prepare 阶段创建新 buffer）。
    // 纹理格式使用 R8Unorm，因此采样类型必须是 float。
    #[texture(0, sample_type = "float")]
    pub grid_texture: Handle<Image>,
}

impl Material2d for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid.wgsl".into()
    }
}

/// 存储材质 Handle 的资源，用于后续更新
#[derive(Resource)]
pub struct GridMaterialHandle(pub Handle<GridMaterial>);

/// 存储 image Handle 的资源，用于后续更新
#[derive(Resource)]
pub struct GridImageHandle(pub Handle<Image>);

/// 初始化网格渲染
pub fn setup_grid_render(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<GridMaterial>>,
    grid: Res<TerritoryGrid>,
) {
    // 创建整数纹理：每像素 1 字节（0..4），带宽更低且更新时不会重建 GPU 资源。
    let size = Extent3d {
        width: grid.width,
        height: grid.height,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        grid.cells().to_vec(),
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    let image_handle = images.add(image);

    // 创建材质
    let material_handle = materials.add(GridMaterial {
        grid_texture: image_handle.clone(),
    });

    // 保存 handle 用于后续更新
    commands.insert_resource(GridMaterialHandle(material_handle.clone()));
    commands.insert_resource(GridImageHandle(image_handle));

    // 创建一个覆盖整个战场的矩形 mesh
    let mesh_handle = meshes.add(Rectangle::new(TERRITORY_LOGIC_WIDTH, TERRITORY_LOGIC_HEIGHT));

    // 在右侧显示网格
    commands.spawn((
        GridRenderer,
        RenderLayers::layer(1),
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::from_translation(Vec3::ZERO),
    ));
}

/// 固定步模拟产出的完整领土画面；满队列时保留最新画面，限制延迟与内存。
#[derive(Resource, Default)]
pub struct GridFrameBuffer {
    frames: VecDeque<Vec<u8>>,
    elapsed: f32,
}

const MAX_BUFFERED_FRAMES: usize = 4;
const GRID_PRESENT_INTERVAL: f32 = 1.0 / 60.0;

impl GridFrameBuffer {
    fn push(&mut self, frame: Vec<u8>) {
        if self.frames.len() == MAX_BUFFERED_FRAMES { self.frames.pop_front(); }
        self.frames.push_back(frame);
    }
}

pub fn capture_grid_frame(mut grid: ResMut<TerritoryGrid>, mut buffer: ResMut<GridFrameBuffer>) {
    if !grid.is_dirty() { return; }
    buffer.push(grid.cells().to_vec());
    grid.clear_dirty();
}

/// 呈现阶段只取一帧。模拟可在一次呈现之间运行多步，队列把突发更新摊开。
pub fn update_grid_render(
    profiler: Res<Profiler>,
    time: Res<Time<Real>>,
    mut buffer: ResMut<GridFrameBuffer>,
    image_handle: Res<GridImageHandle>,
    mut images: ResMut<Assets<Image>>,
    material_handle: Res<GridMaterialHandle>,
    mut materials: ResMut<Assets<GridMaterial>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryUpdateGridRender);
    buffer.elapsed = (buffer.elapsed + time.delta_secs()).min(GRID_PRESENT_INTERVAL);
    if buffer.elapsed < GRID_PRESENT_INTERVAL { return; }
    let Some(frame) = buffer.frames.pop_front() else { return; };
    let Some(image) = images.get_mut(&image_handle.0) else { return; };
    let Some(data) = image.data.as_mut() else { return; };
    if data.len() != frame.len() { return; }
    data.copy_from_slice(&frame);
    buffer.elapsed = 0.0;
    profiler.add_counter(CounterId::TerritoryDirtyTilesUpdated, TOTAL_TILES as u64);
    if let Some(material) = materials.get_mut(&material_handle.0) {
        material.grid_texture = image_handle.0.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_recent_complete_frames_in_order() {
        let mut buffer = GridFrameBuffer::default();
        for value in 0..6 { buffer.push(vec![value]); }
        assert_eq!(buffer.frames.len(), MAX_BUFFERED_FRAMES);
        assert_eq!(buffer.frames.pop_front(), Some(vec![2]));
        assert_eq!(buffer.frames.pop_back(), Some(vec![5]));
    }
}
