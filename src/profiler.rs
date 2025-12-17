use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::cmp::Reverse;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use bevy::time::Real;

#[derive(Clone, Copy, Debug)]
#[repr(u16)]
pub enum ScopeId {
    TerritorySpawnUnitsFromEvents = 0,
    TerritoryMachineGunRotateFire = 1,
    TerritoryUpdateTargetSpatialIndex = 2,
    TerritoryCiwsTargetFire = 3,
    TerritoryBulletMove = 4,
    TerritoryBulletHitTerrain = 5,
    TerritoryBulletHitUnits = 6,
    TerritoryBulletBulletCollision = 7,
    TerritoryBigballOccupyTerritory = 8,
    TerritoryBigballCollision = 9,
    TerritoryCleanupDepletedUnits = 10,
    TerritoryCheckVictory = 11,
    TerritoryContainUnits = 12,
    TerritoryUpdateGridRender = 13,
    TerritoryUpdateBigballValueText = 14,
    TerritorySyncBulletRotationToVelocity = 15,

    PinballCheckMultiplierCollision = 16,
    PinballCheckActionZoneCollision = 17,
    PinballContainMarbles = 18,
    PinballAssistStuckMarbles = 19,
    PinballUpdateMarbleDisplay = 20,
    PinballSyncMarbleTextPosition = 21,
    FixedMainLoop = 22,
}

impl ScopeId {
    pub const COUNT: usize = 23;

    pub fn name(self) -> &'static str {
        match self {
            ScopeId::TerritorySpawnUnitsFromEvents => "territory/spawn_units_from_events",
            ScopeId::TerritoryMachineGunRotateFire => "territory/machine_gun_rotate_fire",
            ScopeId::TerritoryUpdateTargetSpatialIndex => "territory/update_target_spatial_index",
            ScopeId::TerritoryCiwsTargetFire => "territory/ciws_target_fire",
            ScopeId::TerritoryBulletMove => "territory/bullet_move",
            ScopeId::TerritoryBulletHitTerrain => "territory/bullet_hit_terrain",
            ScopeId::TerritoryBulletHitUnits => "territory/bullet_hit_units",
            ScopeId::TerritoryBulletBulletCollision => "territory/bullet_bullet_collision",
            ScopeId::TerritoryBigballOccupyTerritory => "territory/bigball_occupy_territory",
            ScopeId::TerritoryBigballCollision => "territory/bigball_collision",
            ScopeId::TerritoryCleanupDepletedUnits => "territory/cleanup_depleted_units",
            ScopeId::TerritoryCheckVictory => "territory/check_victory",
            ScopeId::TerritoryContainUnits => "territory/contain_units",
            ScopeId::TerritoryUpdateGridRender => "territory/update_grid_render",
            ScopeId::TerritoryUpdateBigballValueText => "territory/update_bigball_value_text",
            ScopeId::TerritorySyncBulletRotationToVelocity => "territory/sync_bullet_rotation_to_velocity",
            ScopeId::PinballCheckMultiplierCollision => "pinball/check_multiplier_collision",
            ScopeId::PinballCheckActionZoneCollision => "pinball/check_action_zone_collision",
            ScopeId::PinballContainMarbles => "pinball/contain_marbles",
            ScopeId::PinballAssistStuckMarbles => "pinball/assist_stuck_marbles",
            ScopeId::PinballUpdateMarbleDisplay => "pinball/update_marble_display",
            ScopeId::PinballSyncMarbleTextPosition => "pinball/sync_marble_text_position",
            ScopeId::FixedMainLoop => "bevy/fixed_main_loop",
        }
    }
}

const ALL_SCOPES: [ScopeId; ScopeId::COUNT] = [
    ScopeId::TerritorySpawnUnitsFromEvents,
    ScopeId::TerritoryMachineGunRotateFire,
    ScopeId::TerritoryUpdateTargetSpatialIndex,
    ScopeId::TerritoryCiwsTargetFire,
    ScopeId::TerritoryBulletMove,
    ScopeId::TerritoryBulletHitTerrain,
    ScopeId::TerritoryBulletHitUnits,
    ScopeId::TerritoryBulletBulletCollision,
    ScopeId::TerritoryBigballOccupyTerritory,
    ScopeId::TerritoryBigballCollision,
    ScopeId::TerritoryCleanupDepletedUnits,
    ScopeId::TerritoryCheckVictory,
    ScopeId::TerritoryContainUnits,
    ScopeId::TerritoryUpdateGridRender,
    ScopeId::TerritoryUpdateBigballValueText,
    ScopeId::TerritorySyncBulletRotationToVelocity,
    ScopeId::PinballCheckMultiplierCollision,
    ScopeId::PinballCheckActionZoneCollision,
    ScopeId::PinballContainMarbles,
    ScopeId::PinballAssistStuckMarbles,
    ScopeId::PinballUpdateMarbleDisplay,
    ScopeId::PinballSyncMarbleTextPosition,
    ScopeId::FixedMainLoop,
];

#[derive(Clone, Copy, Debug)]
#[repr(u16)]
pub enum CounterId {
    TerritoryActionEventsRead = 0,
    TerritoryCollisionStartRead = 1,
    PinballCollisionStartRead = 2,
    TerritoryGridCellWrites = 3,
    TerritoryGridCellsOccupied = 4,
    TerritoryBulletPathPoints = 5,
    TerritoryBulletKernelPointsEst = 6,
}

impl CounterId {
    pub const COUNT: usize = 7;

    pub fn name(self) -> &'static str {
        match self {
            CounterId::TerritoryActionEventsRead => "action_events/read",
            CounterId::TerritoryCollisionStartRead => "territory/collision_start/read",
            CounterId::PinballCollisionStartRead => "pinball/collision_start/read",
            CounterId::TerritoryGridCellWrites => "territory/grid/cell_writes",
            CounterId::TerritoryGridCellsOccupied => "territory/grid/cells_occupied",
            CounterId::TerritoryBulletPathPoints => "territory/bullet/path_points",
            CounterId::TerritoryBulletKernelPointsEst => "territory/bullet/kernel_points_est",
        }
    }
}

const ALL_COUNTERS: [CounterId; CounterId::COUNT] = [
    CounterId::TerritoryActionEventsRead,
    CounterId::TerritoryCollisionStartRead,
    CounterId::PinballCollisionStartRead,
    CounterId::TerritoryGridCellWrites,
    CounterId::TerritoryGridCellsOccupied,
    CounterId::TerritoryBulletPathPoints,
    CounterId::TerritoryBulletKernelPointsEst,
];

#[derive(Resource)]
pub struct Profiler {
    enabled: AtomicBool,
    frame_accounted_ns: AtomicU64,
    frame_update_ns: AtomicU64,
    scopes: [ScopeStats; ScopeId::COUNT],
    counters: [AtomicU64; CounterId::COUNT],
    frame_update_start: std::sync::Mutex<Option<Instant>>,
    fixed_loop_start: std::sync::Mutex<Option<Instant>>,
}

#[derive(Debug)]
struct ScopeStats {
    last_ns: AtomicU64,
    window_sum_ns: AtomicU64,
    window_max_ns: AtomicU64,
    window_calls: AtomicU64,
}

impl Default for Profiler {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            frame_accounted_ns: AtomicU64::new(0),
            frame_update_ns: AtomicU64::new(0),
            scopes: std::array::from_fn(|_| ScopeStats {
                last_ns: AtomicU64::new(0),
                window_sum_ns: AtomicU64::new(0),
                window_max_ns: AtomicU64::new(0),
                window_calls: AtomicU64::new(0),
            }),
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
            frame_update_start: std::sync::Mutex::new(None),
            fixed_loop_start: std::sync::Mutex::new(None),
        }
    }
}

impl Profiler {
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn toggle_enabled(&self) -> bool {
        let next = !self.is_enabled();
        self.enabled.store(next, Ordering::Relaxed);
        next
    }

    pub fn begin_frame(&self) {
        if self.is_enabled() {
            self.frame_accounted_ns.store(0, Ordering::Relaxed);
            for scope in &self.scopes {
                scope.last_ns.store(0, Ordering::Relaxed);
            }
        }

        let mut guard = self.frame_update_start.lock().expect("poisoned Profiler mutex");
        *guard = Some(Instant::now());
    }

    pub fn end_frame(&self) {
        let start = self
            .frame_update_start
            .lock()
            .expect("poisoned Profiler mutex")
            .take();
        if let Some(start) = start {
            self.frame_update_ns
                .store(start.elapsed().as_nanos().min(u64::MAX as u128) as u64, Ordering::Relaxed);
        }
    }

    pub fn fixed_loop_begin(&self) {
        if !self.is_enabled() {
            return;
        }
        let mut guard = self.fixed_loop_start.lock().expect("poisoned Profiler mutex");
        *guard = Some(Instant::now());
    }

    pub fn fixed_loop_end(&self) {
        if !self.is_enabled() {
            return;
        }
        let start = self
            .fixed_loop_start
            .lock()
            .expect("poisoned Profiler mutex")
            .take();
        let Some(start) = start else {
            return;
        };
        self.record(ScopeId::FixedMainLoop, start.elapsed());
    }

    pub fn scope(&self, id: ScopeId) -> ScopeGuard<'_> {
        if !self.is_enabled() {
            return ScopeGuard { profiler: self, id, start: None };
        }
        ScopeGuard {
            profiler: self,
            id,
            start: Some(Instant::now()),
        }
    }

    pub fn add_counter(&self, id: CounterId, value: u64) {
        if !self.is_enabled() {
            return;
        }
        self.counters[id as usize].fetch_add(value, Ordering::Relaxed);
    }

    fn record(&self, id: ScopeId, elapsed: Duration) {
        let ns = elapsed.as_nanos().min(u64::MAX as u128) as u64;
        let scope = &self.scopes[id as usize];
        scope.last_ns.store(ns, Ordering::Relaxed);
        scope.window_sum_ns.fetch_add(ns, Ordering::Relaxed);
        scope.window_calls.fetch_add(1, Ordering::Relaxed);
        scope.window_max_ns.fetch_max(ns, Ordering::Relaxed);
        self.frame_accounted_ns.fetch_add(ns, Ordering::Relaxed);
    }
}

pub struct ScopeGuard<'a> {
    profiler: &'a Profiler,
    id: ScopeId,
    start: Option<Instant>,
}

impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        let Some(start) = self.start.take() else { return; };
        self.profiler.record(self.id, start.elapsed());
    }
}

#[derive(Component)]
struct ProfilerOverlayText;

pub struct ProfilerPlugin;

impl Plugin for ProfilerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Profiler>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(First, profiler_begin_frame)
            .add_systems(FixedFirst, fixed_loop_begin)
            .add_systems(FixedLast, fixed_loop_end)
            .add_systems(Last, (toggle_overlay, position_overlay, update_overlay, profiler_end_frame).chain());
    }
}

fn profiler_begin_frame(profiler: Res<Profiler>) {
    profiler.begin_frame();
}

fn profiler_end_frame(profiler: Res<Profiler>) {
    profiler.end_frame();
}

fn fixed_loop_begin(profiler: Res<Profiler>) {
    profiler.fixed_loop_begin();
}

fn fixed_loop_end(profiler: Res<Profiler>) {
    profiler.fixed_loop_end();
}

fn spawn_overlay(mut commands: Commands, asset_server: Res<AssetServer>) {
    use bevy::camera::visibility::RenderLayers;

    commands.spawn((
        ProfilerOverlayText,
        RenderLayers::layer(0),
        Anchor::TOP_LEFT,
        TextLayout::new_with_justify(Justify::Left),
        Text2d::new(""),
        TextFont {
            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextBackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        Transform::from_translation(Vec3::new(0.0, 0.0, 990.0)),
    ));
}

fn position_overlay(
    profiler: Res<Profiler>,
    mut overlay: Query<&mut Transform, With<ProfilerOverlayText>>,
    cameras: Query<(&Camera, &Projection, &GlobalTransform, Option<&bevy::camera::visibility::RenderLayers>)>,
) {
    if !profiler.is_enabled() {
        return;
    }

    let Ok(mut overlay_transform) = overlay.single_mut() else {
        return;
    };

    let layer0 = bevy::camera::visibility::RenderLayers::layer(0);
    let mut selected: Option<(&Camera, &Projection, &GlobalTransform)> = None;
    for (camera, projection, camera_transform, layers) in cameras.iter() {
        let on_layer0 = layers.map_or(true, |l| l.intersects(&layer0));
        if !on_layer0 {
            continue;
        }
        selected = Some((camera, projection, camera_transform));
        if camera.order == 0 {
            break;
        }
    }
    let Some((camera, projection, camera_transform)) = selected else {
        return;
    };

    let viewport_aspect = camera
        .viewport
        .as_ref()
        .map(|vp| vp.physical_size.x as f32 / vp.physical_size.y as f32)
        .unwrap_or(1.0);

    let (view_w, view_h) = match projection {
        Projection::Orthographic(ortho) => match ortho.scaling_mode {
            bevy::camera::ScalingMode::FixedVertical { viewport_height } => {
                (viewport_height * viewport_aspect, viewport_height)
            }
            bevy::camera::ScalingMode::FixedHorizontal { viewport_width } => {
                (viewport_width, viewport_width / viewport_aspect.max(1e-6))
            }
            _ => (crate::pinball::PINBALL_WIDTH, crate::pinball::PINBALL_HEIGHT),
        },
        _ => (crate::pinball::PINBALL_WIDTH, crate::pinball::PINBALL_HEIGHT),
    };

    let margin_x = 8.0;
    let margin_y = 8.0;
    let half_w = view_w * 0.5;
    let half_h = view_h * 0.5;
    let cam_pos = camera_transform.translation();
    overlay_transform.translation = Vec3::new(
        cam_pos.x - half_w + margin_x,
        cam_pos.y + half_h - margin_y,
        cam_pos.z - 10.0,
    );
}

fn toggle_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    profiler: Res<Profiler>,
    mut text: Query<&mut Visibility, With<ProfilerOverlayText>>,
) {
    if !keys.just_pressed(KeyCode::F3) {
        return;
    }
    let enabled = profiler.toggle_enabled();
    let Ok(mut vis) = text.single_mut() else {
        return;
    };
    *vis = if enabled {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

#[derive(Default)]
struct OverlayCache {
    history: VecDeque<(f64, f64)>, // (t, frame_ms)
    next_render_at: f64,
    next_sample_at: f64,
    snapshot_lines: String,
}

fn update_overlay(
    profiler: Res<Profiler>,
    time: Res<Time<Real>>,
    diagnostics: Res<DiagnosticsStore>,
    mut text: Query<&mut Text2d, With<ProfilerOverlayText>>,
    bullets: Query<Entity, With<crate::territory::Bullet>>,
    bigballs: Query<Entity, With<crate::territory::BigBall>>,
    mut cache: Local<OverlayCache>,
) {
    if !profiler.is_enabled() {
        return;
    }

    let t = time.elapsed_secs_f64();
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    cache.history.push_back((t, frame_ms));
    while let Some((t0, _)) = cache.history.front().copied() {
        if t - t0 <= 1.0 {
            break;
        }
        cache.history.pop_front();
    }

    if t >= cache.next_sample_at {
        cache.next_sample_at = t + 1.0;

        let mut rows: Vec<(ScopeId, u64, u64, u64)> = ALL_SCOPES
            .iter()
            .copied()
            .map(|id| {
                let i = id as usize;
                let scope = &profiler.scopes[i];
                let sum = scope.window_sum_ns.swap(0, Ordering::Relaxed);
                let max = scope.window_max_ns.swap(0, Ordering::Relaxed);
                let calls = scope.window_calls.swap(0, Ordering::Relaxed);
                (id, sum, max, calls)
            })
            .collect();
        rows.sort_by_key(|(_, sum, _, _)| Reverse(*sum));

        let bullets_n = bullets.iter().count();
        let bigballs_n = bigballs.iter().count();

        // Counters (per 1s window)
        let mut counters = Vec::new();
        for id in ALL_COUNTERS {
            let v = profiler.counters[id as usize].swap(0, Ordering::Relaxed);
            if v != 0 {
                counters.push((id, v));
            }
        }

        let mut s = String::new();
        s.push_str(&format!("Entities: Bullet {bullets_n} | BigBall {bigballs_n}\n"));
        for (id, v) in counters {
            s.push_str(&format!("{}: {v}\n", id.name()));
        }
        s.push_str("Top systems (1s sum / 1s max):\n");
        for (id, sum, max, calls) in rows.into_iter().take(8) {
            if calls == 0 {
                continue;
            }
            let sum_ms = sum as f64 / 1_000_000.0;
            let max_ms = max as f64 / 1_000_000.0;
            s.push_str(&format!("  {sum_ms:>6.2} / {max_ms:>6.2}  {}\n", id.name()));
        }

        cache.snapshot_lines = s;
    }

    if t < cache.next_render_at {
        return;
    }
    cache.next_render_at = t + 0.1;

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let max_1s_ms = cache
        .history
        .iter()
        .map(|(_, ms)| *ms)
        .fold(0.0_f64, f64::max);
    let avg_1s_ms = if cache.history.is_empty() {
        0.0
    } else {
        cache.history.iter().map(|(_, ms)| *ms).sum::<f64>() / cache.history.len() as f64
    };

    let accounted_ms = profiler.frame_accounted_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    let update_ms = profiler.frame_update_ns.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    let unaccounted_ms = (update_ms - accounted_ms).max(0.0);
    let total_entities = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0) as u64;

    let mut s = String::new();
    s.push_str(&format!(
        "FPS {fps:>5.1} | frame {frame_ms:>5.2}ms | 1s avg {avg_1s_ms:>5.2}ms | 1s max {max_1s_ms:>5.2}ms\n"
    ));
    s.push_str(&format!(
        "Update {update_ms:>6.2}ms | accounted {accounted_ms:>6.2}ms | other {unaccounted_ms:>6.2}ms | F3 toggle\n"
    ));
        s.push_str(&format!("Entities total {total_entities}\n"));
        s.push_str(&cache.snapshot_lines);

    if let Ok(mut t) = text.single_mut() {
        t.0 = s;
    }
}
