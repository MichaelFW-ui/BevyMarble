use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::colors::TeamColor;
use super::grid::TerritoryGrid;

/// 网格渲染组件
#[derive(Component)]
pub struct GridRenderer;

/// 初始化网格渲染
pub fn setup_grid_render(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    grid: Res<TerritoryGrid>,
) {
    let image = create_grid_image(&grid);
    let image_handle = images.add(image);

    // 在右侧显示网格
    commands.spawn((
        GridRenderer,
        Sprite {
            image: image_handle.clone(),
            custom_size: Some(Vec2::new(800.0, 800.0)),
            ..default()
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 0.0)),
    ));
}

/// 更新网格渲染
pub fn update_grid_render(
    grid: Res<TerritoryGrid>,
    mut images: ResMut<Assets<Image>>,
    query: Query<&Sprite, With<GridRenderer>>,
) {
    if !grid.is_changed() {
        return;
    }

    for sprite in query.iter() {
        if let Some(image) = images.get_mut(&sprite.image) {
            update_grid_image(image, &grid);
        }
    }
}

fn create_grid_image(grid: &TerritoryGrid) -> Image {
    let width = grid.width;
    let height = grid.height;
    let mut data = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let index = ((y * width + x) * 4) as usize;
            let color = match grid.get(x, y) {
                Some(TeamColor::Red) => [230, 50, 50, 255],
                Some(TeamColor::Blue) => [50, 100, 230, 255],
                Some(TeamColor::Green) => [50, 200, 75, 255],
                Some(TeamColor::Yellow) => [230, 215, 50, 255],
                None => [40, 40, 45, 255], // 未占领区域
            };

            data[index..index + 4].copy_from_slice(&color);
        }
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        Default::default(),
    )
}

fn update_grid_image(image: &mut Image, grid: &TerritoryGrid) {
    let width = grid.width;
    let height = grid.height;

    if let Some(data) = &mut image.data {
        for y in 0..height {
            for x in 0..width {
                let index = ((y * width + x) * 4) as usize;
                let color = match grid.get(x, y) {
                    Some(TeamColor::Red) => [230, 50, 50, 255],
                    Some(TeamColor::Blue) => [50, 100, 230, 255],
                    Some(TeamColor::Green) => [50, 200, 75, 255],
                    Some(TeamColor::Yellow) => [230, 215, 50, 255],
                    None => [40, 40, 45, 255],
                };

                data[index..index + 4].copy_from_slice(&color);
            }
        }
    }
}
