use bevy::prelude::*;
use bevy::image::ImageSampler;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;
use bevy::camera::visibility::RenderLayers;

use crate::profiler::{CounterId, Profiler, ScopeId};
use super::grid::{TerritoryGrid, TILE_SIZE, TOTAL_TILES};
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

/// 脏 tile 阈值：超过此比例则使用全量更新
const DIRTY_THRESHOLD_PERCENT: u32 = 25;

/// 更新网格渲染 - 支持 tile 级别增量更新
pub fn update_grid_render(
    profiler: Res<Profiler>,
    mut grid: ResMut<TerritoryGrid>,
    image_handle: Res<GridImageHandle>,
    mut images: ResMut<Assets<Image>>,
    material_handle: Res<GridMaterialHandle>,
    mut materials: ResMut<Assets<GridMaterial>>,
) {
    let _scope = profiler.scope(ScopeId::TerritoryUpdateGridRender);
    
    // 使用显式脏标记，避免 ResMut 借用导致的假阳性
    if !grid.is_dirty() {
        return;
    }

    let Some(image) = images.get_mut(&image_handle.0) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };

    let dirty_count = grid.get_dirty_count();
    let cells = grid.cells();
    let width = grid.width as usize;
    
    // 判断是使用 tile 级别更新还是全量更新
    let dirty_threshold = (TOTAL_TILES as u32 * DIRTY_THRESHOLD_PERCENT) / 100;
    
    if dirty_count <= dirty_threshold {
        // Tile 级别增量更新
        let profiling = profiler.is_enabled();
        let mut tiles_updated = 0u64;
        
        for (tile_x, tile_y) in grid.iter_dirty_tiles() {
            let (x_start, y_start, _x_end, y_end) = TerritoryGrid::tile_cell_range(tile_x, tile_y);
            
            // 复制该 tile 的每一行
            for y in y_start..y_end {
                let row_start = y as usize * width + x_start as usize;
                let row_end = row_start + TILE_SIZE as usize;
                data[row_start..row_end].copy_from_slice(&cells[row_start..row_end]);
            }
            
            if profiling {
                tiles_updated += 1;
            }
        }
        
        profiler.add_counter(CounterId::TerritoryDirtyTilesUpdated, tiles_updated);
    } else {
        // 全量更新（脏 tile 过多）
        if data.len() == cells.len() {
            data.copy_from_slice(cells);
        } else {
            *data = cells.to_vec();
        }
        
        profiler.add_counter(CounterId::TerritoryDirtyTilesUpdated, TOTAL_TILES as u64);
    }
    
    // 清空脏标记
    grid.clear_dirty();

    // 关键：Image 被修改后，渲染端会重建 `GpuImage`（新的 texture/view）。
    // 但材质的 bind group 不会因为 Image 变化自动重建，所以需要触碰材质触发重新 prepare。
    if let Some(material) = materials.get_mut(&material_handle.0) {
        material.grid_texture = image_handle.0.clone();
    }
}
