//! 编辑器和游戏共用的弹珠机数据与文件格式。
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use super::components::ActionZoneType;
use super::utils::MAX_VALUE;
use crate::colors::TeamColor;

pub const PROFILE_VERSION: u32 = 1;

pub fn profile_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/pinball/profiles.json")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectShape {
    Rectangle,
    Ellipse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectKind {
    Wall,
    Peg,
    Multiplier {
        factor: u64,
        reset_position: bool,
    },
    Action {
        action: ActionZoneType,
        value_scale: f32,
        reset_position: bool,
    },
    Boost {
        velocity: [f32; 2],
    },
    Spawn {
        team: TeamColor,
    },
}

impl ObjectKind {
    pub fn label(&self) -> String {
        match self {
            Self::Wall => "Wall".into(),
            Self::Peg => "Peg".into(),
            Self::Multiplier { factor, .. } => format!("x{factor}"),
            Self::Action { action, .. } => match action {
                ActionZoneType::BigBall => "Big Ball",
                ActionZoneType::Shield => "Shield",
                ActionZoneType::MachineGun => "MG",
                ActionZoneType::CIWS => "CIWS",
            }
            .into(),
            Self::Boost { .. } => "Boost".into(),
            Self::Spawn { team } => format!("{team:?}"),
        }
    }

    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Wall | Self::Peg)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinballObject {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub position: [f32; 2],
    /// 完整宽高；椭圆使用直径。
    pub size: [f32; 2],
    pub scale: [f32; 2],
    /// 角度，单位为度。
    pub rotation: f32,
    pub shape: ObjectShape,
    pub color: [f32; 4],
    pub restitution: f32,
    pub friction: f32,
    pub kind: ObjectKind,
}

impl PinballObject {
    pub fn new(id: u64, kind: ObjectKind, position: [f32; 2], size: [f32; 2]) -> Self {
        let shape = if matches!(kind, ObjectKind::Peg | ObjectKind::Spawn { .. }) {
            ObjectShape::Ellipse
        } else {
            ObjectShape::Rectangle
        };
        let color = match &kind {
            ObjectKind::Wall => [0.3, 0.3, 0.4, 1.0],
            ObjectKind::Peg => [0.5, 0.5, 0.6, 1.0],
            ObjectKind::Multiplier { .. } => [0.2, 0.8, 0.2, 0.5],
            ObjectKind::Action { .. } => [0.3, 0.5, 0.9, 0.6],
            ObjectKind::Boost { .. } => [0.3, 0.9, 0.9, 0.5],
            ObjectKind::Spawn { team } => team.to_color().to_srgba().to_f32_array(),
        };
        Self {
            id,
            name: format!("{} {id}", kind.label()),
            enabled: true,
            position,
            size,
            scale: [1.0; 2],
            rotation: 0.0,
            shape,
            color,
            restitution: 0.0,
            friction: 0.5,
            kind,
        }
    }

    pub fn dimensions(&self) -> Vec2 {
        Vec2::from_array(self.size) * Vec2::from_array(self.scale)
    }

    pub fn contains(&self, point: Vec2) -> bool {
        let local = Mat2::from_angle(-self.rotation.to_radians())
            * (point - Vec2::from_array(self.position));
        let normalized = local / (self.dimensions() * 0.5);
        match self.shape {
            ObjectShape::Rectangle => normalized.abs().max_element() <= 1.0,
            ObjectShape::Ellipse => normalized.length_squared() <= 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarbleSettings {
    pub initial_value: u64,
    pub restitution: f32,
    pub friction: f32,
    pub spawn_speed: f32,
    pub rescue_speed: f32,
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinballProfile {
    pub id: String,
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub gravity: [f32; 2],
    pub marble: MarbleSettings,
    pub objects: Vec<PinballObject>,
}

impl Default for PinballProfile {
    fn default() -> Self {
        let mut profile = Self {
            id: "classic".into(),
            name: "Classic".into(),
            width: 400.0,
            height: 800.0,
            gravity: [0.0, -490.0],
            marble: MarbleSettings {
                initial_value: 1000,
                restitution: 0.6,
                friction: 0.3,
                spawn_speed: 50.0,
                rescue_speed: 420.0,
            },
            objects: Vec::new(),
        };
        let mut add = |kind, position, size| {
            let id = profile.objects.len() as u64 + 1;
            profile
                .objects
                .push(PinballObject::new(id, kind, position, size));
        };
        for (position, size) in [
            ([-200.0, 0.0], [10.0, 800.0]),
            ([200.0, 0.0], [10.0, 800.0]),
            ([0.0, -400.0], [400.0, 10.0]),
            ([0.0, 400.0], [400.0, 10.0]),
        ] {
            add(ObjectKind::Wall, position, size);
        }
        for (y, xs) in [
            (250.0, vec![-120.0, -40.0, 40.0, 120.0]),
            (200.0, vec![-80.0, 0.0, 80.0]),
            (150.0, vec![-120.0, -40.0, 40.0, 120.0]),
            (50.0, vec![-140.0, -60.0, 60.0, 140.0]),
            (0.0, vec![-100.0, -20.0, 20.0, 100.0]),
            (-100.0, vec![-140.0, -60.0, 60.0, 140.0]),
            (-150.0, vec![-100.0, -20.0, 20.0, 100.0]),
            (-200.0, vec![-140.0, -60.0, 60.0, 140.0]),
        ] {
            for x in xs {
                add(ObjectKind::Peg, [x, y], [16.0; 2]);
            }
        }
        for (position, factor, width) in [
            ([-100.0, -50.0], 2, 80.0),
            ([100.0, -50.0], 2, 80.0),
            ([0.0, -50.0], 4, 80.0),
            ([0.0, 100.0], 8, 48.0),
        ] {
            add(
                ObjectKind::Multiplier {
                    factor,
                    reset_position: true,
                },
                position,
                [width, 40.0],
            );
        }
        for x in [-140.0, -100.0, -60.0, -20.0, 20.0, 60.0, 100.0, 140.0] {
            add(ObjectKind::Peg, [x, -20.0], [16.0; 2]);
        }
        for (action, x) in [
            (ActionZoneType::BigBall, -142.5),
            (ActionZoneType::Shield, -47.5),
            (ActionZoneType::MachineGun, 47.5),
            (ActionZoneType::CIWS, 142.5),
        ] {
            add(
                ObjectKind::Action {
                    action,
                    value_scale: 1.0,
                    reset_position: true,
                },
                [x, -350.0],
                [95.0, 80.0],
            );
        }
        for (team, x) in [
            (TeamColor::Red, -60.0),
            (TeamColor::Blue, -20.0),
            (TeamColor::Green, 20.0),
            (TeamColor::Yellow, 60.0),
        ] {
            add(ObjectKind::Spawn { team }, [x, 350.0], [12.0; 2]);
        }
        // 保留原版的区域和格点配色。
        for object in &mut profile.objects {
            object.color = match object.kind {
                ObjectKind::Multiplier { factor: 4, .. } => [0.8, 0.6, 0.2, 0.5],
                ObjectKind::Multiplier { factor: 8, .. } => [0.9, 0.2, 0.8, 0.5],
                ObjectKind::Action {
                    action: ActionZoneType::BigBall,
                    ..
                } => [0.9, 0.3, 0.3, 0.6],
                ObjectKind::Action {
                    action: ActionZoneType::MachineGun,
                    ..
                } => [0.9, 0.9, 0.3, 0.6],
                ObjectKind::Action {
                    action: ActionZoneType::CIWS,
                    ..
                } => [0.9, 0.5, 0.2, 0.6],
                ObjectKind::Peg if object.position[1] == -20.0 => [0.7, 0.3, 0.3, 1.0],
                _ => object.color,
            };
        }
        profile
    }
}

fn finite_range(value: f32, min: f32, max: f32) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

impl PinballProfile {
    pub fn next_object_id(&self) -> u64 {
        self.objects
            .iter()
            .map(|object| object.id)
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err("弹珠机 ID 和名称不能为空".into());
        }
        if ![self.width, self.height]
            .into_iter()
            .all(|v| finite_range(v, 100.0, 10000.0))
        {
            return Err("场地宽高需要在 100 到 10000 之间".into());
        }
        if !self
            .gravity
            .into_iter()
            .all(|v| finite_range(v, -10000.0, 10000.0))
            || !(1..=MAX_VALUE).contains(&self.marble.initial_value)
            || !finite_range(self.marble.restitution, 0.0, 1.0)
            || !finite_range(self.marble.friction, 0.0, 10.0)
            || !finite_range(self.marble.spawn_speed, 0.0, 10000.0)
            || !finite_range(self.marble.rescue_speed, 0.0, 10000.0)
        {
            return Err("重力或弹珠参数超出允许范围".into());
        }
        let mut ids = HashSet::new();
        let mut spawns = [0; 4];
        for object in &self.objects {
            if object.id == 0 || object.id == u64::MAX || !ids.insert(object.id) {
                return Err("对象 ID 必须是唯一的正整数".into());
            }
            if object.name.trim().is_empty()
                || !object
                    .position
                    .into_iter()
                    .all(|v| finite_range(v, -20000.0, 20000.0))
                || !object
                    .size
                    .into_iter()
                    .all(|v| finite_range(v, 0.1, 10000.0))
                || !object
                    .scale
                    .into_iter()
                    .all(|v| finite_range(v, 0.01, 100.0))
                || object.dimensions().max_element() > 20000.0
                || !object.rotation.is_finite()
                || !object.color.into_iter().all(|v| finite_range(v, 0.0, 1.0))
                || !finite_range(object.restitution, 0.0, 1.0)
                || !finite_range(object.friction, 0.0, 10.0)
            {
                return Err(format!("对象 {} 的几何或材质参数无效", object.name));
            }
            match object.kind {
                ObjectKind::Multiplier { factor, .. } if !(1..=MAX_VALUE).contains(&factor) => {
                    return Err(format!("{} 的倍率无效", object.name));
                }
                ObjectKind::Action { value_scale, .. }
                    if !finite_range(value_scale, 0.01, 1000.0) =>
                {
                    return Err(format!("{} 的行动数值系数无效", object.name));
                }
                ObjectKind::Boost { velocity }
                    if !velocity
                        .into_iter()
                        .all(|v| finite_range(v, -10000.0, 10000.0)) =>
                {
                    return Err(format!("{} 的加速参数无效", object.name));
                }
                ObjectKind::Spawn { team } if object.enabled => {
                    spawns[team.index()] += 1;
                    if object.position[0].abs() > self.width / 2.0
                        || object.position[1].abs() > self.height / 2.0
                    {
                        return Err(format!("{} 的出生点需要放在场地内", object.name));
                    }
                }
                _ => {}
            }
        }
        if spawns != [1; 4] {
            return Err("每个队伍需要且只能有一个启用的出生点".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileLibrary {
    pub version: u32,
    pub default_profile: String,
    pub profiles: Vec<PinballProfile>,
}

impl Default for ProfileLibrary {
    fn default() -> Self {
        Self {
            version: PROFILE_VERSION,
            default_profile: "classic".into(),
            profiles: vec![PinballProfile::default()],
        }
    }
}

impl ProfileLibrary {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != PROFILE_VERSION {
            return Err(format!("不支持的 profile 版本：{}", self.version));
        }
        let mut ids = HashSet::new();
        for profile in &self.profiles {
            profile
                .validate()
                .map_err(|error| format!("{}：{error}", profile.name))?;
            if !ids.insert(&profile.id) {
                return Err("弹珠机 ID 不能重复".into());
            }
        }
        if !ids.contains(&self.default_profile) {
            return Err("默认弹珠机不存在".into());
        }
        Ok(())
    }

    pub fn active(&self) -> &PinballProfile {
        self.profiles
            .iter()
            .find(|profile| profile.id == self.default_profile)
            .expect("使用 active 前须验证 profile")
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let content =
            fs::read(path).map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
        let library: Self = serde_json::from_slice(&content)
            .map_err(|error| format!("profile 格式错误：{error}"))?;
        library.validate()?;
        Ok(library)
    }

    /// 写入临时文件后原子替换，保留上一次完整配置直到保存成功。
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let mut content = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        content.push(b'\n');
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, content).map_err(|error| format!("保存失败：{error}"))?;
        fs::rename(&temporary, path).map_err(|error| format!("替换 profile 文件失败：{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_library_is_valid_and_keeps_classic_layout() {
        let library = ProfileLibrary::load(&profile_path()).unwrap();
        assert_eq!(library.active(), &PinballProfile::default());
        assert!(library.profiles.len() >= 2);
        assert_eq!(library.active().objects.len(), 55);
    }

    #[test]
    fn profiles_round_trip_with_all_effects_and_transforms() {
        let mut library = ProfileLibrary::default();
        let object = &mut library.profiles[0].objects[5];
        object.rotation = 37.0;
        object.scale = [1.4, 0.7];
        object.kind = ObjectKind::Boost {
            velocity: [120.0, 300.0],
        };
        let path =
            std::env::temp_dir().join(format!("bevymarble-profile-{}.json", std::process::id()));
        library.save(&path).unwrap();
        assert_eq!(ProfileLibrary::load(&path).unwrap(), library);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_profiles_cannot_replace_saved_data() {
        let mut library = ProfileLibrary::default();
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            library.profiles[0].objects[0].scale[0] = invalid;
            assert!(library.validate().is_err());
        }
        library = ProfileLibrary::default();
        library.default_profile = "missing".into();
        assert!(library.validate().is_err());
        library = ProfileLibrary::default();
        library.profiles[0].objects.pop();
        assert!(library.validate().is_err());
        library = ProfileLibrary::default();
        library.profiles[0].objects[1].id = library.profiles[0].objects[0].id;
        assert!(library.validate().is_err());
        library = ProfileLibrary::default();
        library.profiles.push(library.profiles[0].clone());
        assert!(library.validate().is_err());
    }

    #[test]
    fn picking_respects_rotation_and_nonuniform_scale() {
        let mut object = PinballObject::new(1, ObjectKind::Peg, [10.0, 20.0], [40.0, 10.0]);
        object.rotation = 90.0;
        object.scale = [2.0, 1.0];
        assert!(object.contains(Vec2::new(10.0, 55.0)));
        assert!(!object.contains(Vec2::new(18.0, 20.0)));
    }
}
