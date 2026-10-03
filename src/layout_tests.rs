use super::*;
use bevy::camera::RenderTarget;
use bevy::render::render_resource::TextureFormat;
use bevy::render::texture::ManualTextureViews;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PrimaryWindow, WindowCreated, WindowResized, WindowScaleFactorChanged};

#[test]
fn resize_and_dpi_changes_keep_viewports_inside_window() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_resource::<PinballProfile>()
        .init_asset::<Image>()
        .init_resource::<ManualTextureViews>()
        .add_message::<WindowCreated>()
        .add_message::<WindowResized>()
        .add_message::<WindowScaleFactorChanged>()
        .add_systems(Startup, setup)
        .add_systems(Update, update_game_viewports)
        .add_systems(PostUpdate, bevy::render::camera::camera_system);
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    for (width, height, scale) in [
        (1400.0, 900.0, 1.0),
        (1000.0, 700.0, 1.0),
        (2400.0, 800.0, 1.0),
        (900.0, 1400.0, 1.0),
        (1400.0, 900.0, 2.0),
        (1400.0, 900.0, 1.0),
        (0.0, 0.0, 1.0),
        (1400.0, 900.0, 1.0),
    ] {
        let mut entity = app.world_mut().entity_mut(window);
        let mut component = entity.get_mut::<Window>().unwrap();
        let previous_scale = component.scale_factor();
        component.resolution.set_scale_factor(scale);
        component.resolution.set(width, height);
        let physical_size = component.resolution.physical_size();
        if previous_scale != scale {
            app.world_mut().write_message(WindowScaleFactorChanged {
                window,
                scale_factor: scale as f64,
            });
        }
        app.world_mut().write_message(WindowResized {
            window,
            width,
            height,
        });
        app.update();
        app.update();
        let mut cameras = app.world_mut().query::<(&Camera, &Projection)>();
        let mut total_width = 0;
        for (camera, projection) in cameras.iter(app.world()) {
            let active = physical_size.min_element() > 0;
            assert_eq!(camera.is_active, active);
            if !active {
                continue;
            }
            let viewport = camera.viewport.as_ref().unwrap();
            assert_eq!(viewport.physical_position.y, 0);
            assert_eq!(viewport.physical_size.y, physical_size.y);
            assert!(viewport.physical_position.x + viewport.physical_size.x <= physical_size.x);
            total_width += viewport.physical_size.x;
            let Projection::Orthographic(projection) = projection else {
                panic!("需要正交相机")
            };
            let bounds = if camera.order == 0 {
                Vec2::new(205.0, 405.0)
            } else {
                Vec2::splat(512.0)
            };
            assert!(
                projection.area.min.cmplt(-bounds).all(),
                "场地外框被裁切：{:?}",
                projection.area
            );
            assert!(
                projection.area.max.cmpgt(bounds).all(),
                "场地外框被裁切：{:?}",
                projection.area
            );
        }
        if physical_size.min_element() > 0 {
            assert_eq!(total_width, physical_size.x, "DPI 更新后视口未完整覆盖窗口");
        }
    }
}

#[derive(Resource, Default)]
struct CapturedFrame(Option<Image>);

fn render_app() -> App {
    let mut app = App::new();
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
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(Gravity(Vec2::NEG_Y * 490.0))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
    .init_resource::<bevymarble::profiler::Profiler>()
    .init_resource::<CapturedFrame>()
    .add_message::<ActionEvent>()
    .add_message::<VictoryEvent>()
    .add_message::<UnitDestroyedEvent>()
    .add_plugins(PinballPlugin)
    .add_plugins(TerritoryPlugin)
    .add_systems(Startup, (setup, retarget_cameras).chain())
    .add_systems(Update, update_game_viewports)
    .add_systems(PostStartup, paint_test_grid);
    app.world_mut().spawn((
        Window {
            resolution: (1400, 900).into(),
            ..default()
        },
        PrimaryWindow,
    ));
    let image = Image::new_target_texture(1400, 900, TextureFormat::Rgba8UnormSrgb);
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    app.world_mut().insert_resource(RenderImage(target));
    app.finish();
    app.cleanup();
    app
}

#[derive(Resource)]
struct RenderImage(Handle<Image>);

fn retarget_cameras(target: Res<RenderImage>, mut cameras: Query<&mut Camera>) {
    for mut camera in &mut cameras {
        camera.target = RenderTarget::Image(target.0.clone().into());
    }
}

// 四个大片领土能检验实际着色器、纹理采样、Y 轴方向和两层画面合成。
fn paint_test_grid(mut images: ResMut<Assets<Image>>) {
    for (_, image) in images.iter_mut() {
        if image.texture_descriptor.format == TextureFormat::R8Unorm {
            let data = image.data.as_mut().unwrap();
            for y in 0..1024 {
                for x in 0..1024 {
                    data[y * 1024 + x] = match (x < 512, y < 512) {
                        (true, true) => 1,
                        (true, false) => 2,
                        (false, false) => 3,
                        (false, true) => 4,
                    };
                }
            }
        }
    }
}

fn capture(app: &mut App, name: &str) -> Image {
    for _ in 0..100 {
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
    // 截图目标包含相机的 DPI；仅使用图片 handle 会默认取 1 倍 DPI。
    let mut cameras = app.world_mut().query::<&Camera>();
    let target = cameras.iter(app.world()).next().unwrap().target.clone();
    app.world_mut().spawn(Screenshot(target)).observe(
        |event: On<ScreenshotCaptured>, mut captured: ResMut<CapturedFrame>| {
            captured.0 = Some(event.image.clone());
        },
    );
    for _ in 0..100 {
        app.update();
        if let Some(image) = app.world_mut().resource_mut::<CapturedFrame>().0.take() {
            if let Some(directory) = std::env::var_os("BEVY_LAYOUT_PREVIEW_DIR") {
                let path = std::path::PathBuf::from(directory).join(format!("{name}.png"));
                image
                    .clone()
                    .try_into_dynamic()
                    .unwrap()
                    .to_rgb8()
                    .save(path)
                    .unwrap();
            }
            return image;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("GPU 未返回布局画面");
}

fn pixel_at(app: &mut App, image: &Image, order: isize, position: Vec2) -> [u8; 4] {
    let mut cameras = app.world_mut().query::<(&Camera, &Projection)>();
    let (camera, projection) = cameras
        .iter(app.world())
        .find(|(camera, _)| camera.order == order)
        .unwrap();
    let viewport = camera.viewport.as_ref().unwrap();
    let Projection::Orthographic(projection) = projection else {
        panic!("需要正交相机")
    };
    let x = viewport.physical_position.x as f32
        + (position.x - projection.area.min.x) / projection.area.width()
            * viewport.physical_size.x as f32;
    let y = viewport.physical_position.y as f32
        + (projection.area.max.y - position.y) / projection.area.height()
            * viewport.physical_size.y as f32;
    image
        .clone()
        .try_into_dynamic()
        .unwrap()
        .to_rgba8()
        .get_pixel(x as u32, y as u32)
        .0
}

fn assert_scene_visible(app: &mut App, image: &Image) {
    let red = pixel_at(app, image, 1, Vec2::new(-256.0, -256.0));
    let blue = pixel_at(app, image, 1, Vec2::new(-256.0, 256.0));
    let green = pixel_at(app, image, 1, Vec2::new(256.0, 256.0));
    let yellow = pixel_at(app, image, 1, Vec2::new(256.0, -256.0));
    assert!(
        red[0] > red[1] + 30 && red[0] > red[2] + 30,
        "红色地面未绘制：{red:?}"
    );
    assert!(
        blue[2] > blue[0] + 30 && blue[2] > blue[1] + 30,
        "蓝色地面未绘制：{blue:?}"
    );
    assert!(
        green[1] > green[0] + 30 && green[1] > green[2] + 30,
        "绿色地面未绘制：{green:?}"
    );
    assert!(
        yellow[0] > yellow[2] + 30 && yellow[1] > yellow[2] + 30,
        "黄色地面未绘制：{yellow:?}"
    );
    let peg = pixel_at(app, image, 0, Vec2::new(-120.0, 250.0));
    assert!(
        peg[0] > 100 && peg[1] > 100 && peg[2] > 140,
        "弹珠机被另一台相机覆盖：{peg:?}"
    );
    let pixels = image.clone().try_into_dynamic().unwrap().to_rgba8();
    let background = [43, 44, 47, 255];
    for (x, y) in [
        (1, pixels.height() / 2),
        (pixels.width() - 2, pixels.height() / 2),
        (pixels.width() / 5, 1),
        (pixels.width() / 5, pixels.height() - 2),
    ] {
        assert_eq!(
            pixels.get_pixel(x, y).0,
            background,
            "窗口边缘缺少留白：({x}, {y})"
        );
    }
}

#[test]
#[ignore = "requires a GPU adapter; renders the actual game to an image without opening a window"]
fn game_layout_keeps_terrain_and_pinball_visible() {
    let mut app = render_app();
    for (width, height, scale, name) in [
        (1400, 900, 1.0, "layout"),
        (1000, 700, 1.0, "layout-small"),
        (1800, 700, 1.0, "layout-wide"),
        (1400, 900, 2.0, "layout-retina"),
    ] {
        let mut windows = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        let mut window = windows.single_mut(app.world_mut()).unwrap();
        window.resolution.set_scale_factor(scale);
        window.resolution.set(width as f32, height as f32);
        let target = app.world().resource::<RenderImage>().0.clone();
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&target)
            .unwrap()
            .resize(bevy::render::render_resource::Extent3d {
                width: (width as f32 * scale) as u32,
                height: (height as f32 * scale) as u32,
                depth_or_array_layers: 1,
            });
        let mut cameras = app.world_mut().query::<&mut Camera>();
        for mut camera in cameras.iter_mut(app.world_mut()) {
            if let RenderTarget::Image(target) = &mut camera.target {
                target.scale_factor = bevy::math::FloatOrd(scale);
            }
        }
        let image = capture(&mut app, name);
        assert_scene_visible(&mut app, &image);
    }
}
