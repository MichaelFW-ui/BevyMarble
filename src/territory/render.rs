use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderStorageBuffer;
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;
use bevy::camera::visibility::RenderLayers;

use super::grid::TerritoryGrid;
use super::coords::{TERRITORY_LOGIC_HEIGHT, TERRITORY_LOGIC_WIDTH};

/// 网格渲染组件
#[derive(Component)]
pub struct GridRenderer;

/// GPU 网格材质
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GridMaterial {
    #[storage(0, read_only)]
    pub grid_buffer: Handle<ShaderStorageBuffer>,
    #[uniform(1)]
    pub grid_size: GridSize,
}

#[derive(ShaderType, Debug, Clone, Copy)]
pub struct GridSize {
    pub width: u32,
    pub height: u32,
}

impl Material2d for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/grid.wgsl".into()
    }
}

/// 存储材质 Handle 的资源，用于后续更新
#[derive(Resource)]
pub struct GridMaterialHandle(pub Handle<GridMaterial>);

/// 存储 buffer Handle 的资源，用于后续更新
#[derive(Resource)]
pub struct GridBufferHandle(pub Handle<ShaderStorageBuffer>);

/// 将 TerritoryGrid 转换为 GPU buffer 数据
fn grid_to_buffer_data(grid: &TerritoryGrid) -> Vec<u32> {
    grid.cells().to_vec()
}

/// 初始化网格渲染
pub fn setup_grid_render(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut materials: ResMut<Assets<GridMaterial>>,
    grid: Res<TerritoryGrid>,
) {
    // 创建 storage buffer
    let buffer_data = grid_to_buffer_data(&grid);
    let buffer_handle = buffers.add(ShaderStorageBuffer::from(buffer_data));

    // 创建材质
    let material_handle = materials.add(GridMaterial {
        grid_buffer: buffer_handle.clone(),
        grid_size: GridSize {
            width: grid.width,
            height: grid.height,
        },
    });

    // 保存 handle 用于后续更新
    commands.insert_resource(GridMaterialHandle(material_handle.clone()));
    commands.insert_resource(GridBufferHandle(buffer_handle));

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

/// 更新网格渲染 - 替换整个 buffer
pub fn update_grid_render(
    grid: Res<TerritoryGrid>,
    buffer_handle: Res<GridBufferHandle>,
    material_handle: Res<GridMaterialHandle>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut materials: ResMut<Assets<GridMaterial>>,
) {
    if !grid.is_changed() {
        return;
    }

    // 创建新的 buffer 数据
    let buffer_data = grid_to_buffer_data(&grid);

    // 更新 buffer asset（避免替换 handle/触发额外资产管理开销）
    if let Some(storage) = buffers.get_mut(&buffer_handle.0) {
        storage.set_data(buffer_data);
    } else {
        let _ = buffers.insert(&buffer_handle.0, ShaderStorageBuffer::from(buffer_data));
    }

    // 触发材质更新
    if let Some(material) = materials.get_mut(&material_handle.0) {
        // 触发变化检测
        material.grid_buffer = buffer_handle.0.clone();
    }
}
