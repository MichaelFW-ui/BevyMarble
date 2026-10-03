use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, TexelCopyBufferLayout, TextureDimension, TextureFormat,
};
use bevy::render::renderer::RenderQueue;
use bevy::render::texture::GpuImage;
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;
use std::sync::Arc;

use super::coords::{TERRITORY_LOGIC_HEIGHT, TERRITORY_LOGIC_WIDTH};
use crate::profiler::{CounterId, Profiler, ScopeId};
use super::grid::{TerritoryGrid, TOTAL_TILES};

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

/// 主世界准备一张逻辑画面，渲染世界将它写入持久 GPU 纹理。
#[derive(Resource, Clone, ExtractResource)]
pub struct GridUpload {
    image: Handle<Image>,
    data: Arc<Vec<u8>>,
    generation: u64,
}

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
    #[cfg(test)]
    {
        image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
    }
    let image_handle = images.add(image);

    // 创建材质
    let material_handle = materials.add(GridMaterial {
        grid_texture: image_handle.clone(),
    });

    // 保存 handle 用于后续更新
    commands.insert_resource(GridUpload {
        image: image_handle,
        data: Arc::default(),
        generation: 0,
    });

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

/// 多个固定步合并为一次呈现，在 Update 中直接采集最新网格。
#[derive(Resource, Default)]
pub struct GridFrameBuffer {
    pending: bool,
    dirty_tiles: u32,
}

pub fn capture_grid_frame(mut grid: ResMut<TerritoryGrid>, mut buffer: ResMut<GridFrameBuffer>) {
    if !grid.is_dirty() { return; }
    buffer.pending = true;
    buffer.dirty_tiles = buffer.dirty_tiles.saturating_add(grid.get_dirty_count());
    grid.clear_dirty();
}

/// 每次呈现使用最新状态；复用无共享的 CPU 缓冲，提取阶段只克隆 Arc。
pub fn update_grid_render(
    profiler: Res<Profiler>,
    grid: Res<TerritoryGrid>,
    mut buffer: ResMut<GridFrameBuffer>,
    mut upload: ResMut<GridUpload>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryUpdateGridRender);
    if !buffer.pending { return; }
    if let Some(data) = Arc::get_mut(&mut upload.data) {
        data.clear();
        data.extend_from_slice(grid.cells());
    } else {
        upload.data = Arc::new(grid.cells().to_vec());
    }
    upload.generation = upload.generation.wrapping_add(1);
    profiler.add_counter(CounterId::TerritoryDirtyTilesUpdated, buffer.dirty_tiles.min(TOTAL_TILES as u32) as u64);
    buffer.pending = false;
    buffer.dirty_tiles = 0;
}

/// 保留纹理与材质绑定，只把新画面写入已经创建的 GPU 纹理。
pub fn upload_grid_texture(
    upload: Res<GridUpload>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    queue: Res<RenderQueue>,
    mut last_generation: Local<u64>,
) {
    if upload.generation == *last_generation || upload.data.is_empty() { return; }
    let Some(gpu_image) = gpu_images.get(&upload.image) else { return; };
    let width = gpu_image.size.width;
    let height = gpu_image.size.height;
    if upload.data.len() != (width * height) as usize { return; }
    queue.write_texture(
        gpu_image.texture.as_image_copy(),
        &upload.data,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width),
            rows_per_image: Some(height),
        },
        gpu_image.size,
    );
    *last_generation = upload.generation;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catch_up_presents_latest_grid_without_waiting_for_a_timer() {
        let mut app = App::new();
        app.init_resource::<Profiler>()
            .insert_resource(TerritoryGrid::new(1024, 1024))
            .init_resource::<GridFrameBuffer>()
            .insert_resource(GridUpload { image: Handle::default(), data: Arc::default(), generation: 0 })
            .add_systems(FixedUpdate, capture_grid_frame)
            .add_systems(Update, update_grid_render);
        for team in [crate::colors::TeamColor::Red, crate::colors::TeamColor::Blue] {
            app.world_mut().resource_mut::<TerritoryGrid>().set(512, 512, Some(team));
            app.world_mut().run_schedule(FixedUpdate);
        }
        app.update();
        let upload = app.world().resource::<GridUpload>();
        assert_eq!(upload.data[512 * 1024 + 512], crate::colors::TeamColor::Blue.to_id());
        assert_eq!(upload.generation, 1);
        let extracted = GridUpload::extract_resource(upload);
        assert!(Arc::ptr_eq(&upload.data, &extracted.data));
        app.update();
        assert_eq!(app.world().resource::<GridUpload>().generation, 1);
        // 下次写入保持渲染世界已提取的画面有效。
        app.world_mut().resource_mut::<TerritoryGrid>().set(512, 512, Some(crate::colors::TeamColor::Green));
        app.world_mut().run_schedule(FixedUpdate);
        app.update();
        assert_eq!(extracted.data[512 * 1024 + 512], crate::colors::TeamColor::Blue.to_id());
        assert_eq!(app.world().resource::<GridUpload>().data[512 * 1024 + 512], crate::colors::TeamColor::Green.to_id());
    }
}

#[cfg(test)]
mod headless_gpu_tests {
    use super::*;
    use bevy::app::PluginGroup;
    use bevy::render::extract_resource::ExtractResourcePlugin;
    use bevy::render::{Render, RenderApp, RenderSystems};
    use bevy::sprite_render::Material2dPlugin;
    use bevy::time::TimeUpdateStrategy;
    use bevy::render::render_resource::{BufferDescriptor, BufferUsages, MapMode, PollType, TexelCopyBufferInfo};
    use bevy::render::renderer::RenderDevice;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a GPU adapter; opens no window"]
    fn persistent_grid_texture_is_prepared_without_a_window() {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: None, ..default() })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
        )
        .add_plugins(Material2dPlugin::<GridMaterial>::default())
        .add_plugins(ExtractResourcePlugin::<GridUpload>::default())
        .insert_resource(TerritoryGrid::new(1024, 1024))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(20)))
        .init_resource::<GridFrameBuffer>()
        .init_resource::<Profiler>()
        .add_systems(Startup, setup_grid_render)
        .add_systems(Update, (capture_grid_frame, update_grid_render).chain());
        app.get_sub_app_mut(RenderApp).unwrap()
            .add_systems(Render, upload_grid_texture.in_set(RenderSystems::Prepare));
        app.finish();
        app.cleanup();
        app.update();
        app.world_mut().resource_mut::<TerritoryGrid>().set(512, 512, Some(crate::colors::TeamColor::Red));
        app.update();
        let handle = app.world().resource::<GridUpload>().image.clone();
        let texture_id = app.sub_app(RenderApp).world()
            .resource::<RenderAssets<GpuImage>>().get(&handle).unwrap().texture.id();
        app.update();
        let current_id = app.sub_app(RenderApp).world()
            .resource::<RenderAssets<GpuImage>>().get(&handle).unwrap().texture.id();
        assert_eq!(texture_id, current_id);
        let render_world = app.sub_app(RenderApp).world();
        let gpu_image = render_world.resource::<RenderAssets<GpuImage>>().get(&handle).unwrap();
        let device = render_world.resource::<RenderDevice>();
        let queue = render_world.resource::<RenderQueue>();
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("grid readback"),
            size: 1024 * 1024,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            gpu_image.texture.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(1024), rows_per_image: Some(1024) },
            },
            gpu_image.size,
        );
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(MapMode::Read, move |result| { sender.send(result).unwrap(); });
        device.poll(PollType::Wait).unwrap();
        receiver.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range();
        assert_eq!(pixels[512 * 1024 + 512], crate::colors::TeamColor::Red.to_id());
        drop(pixels);
        buffer.unmap();
    }
}
