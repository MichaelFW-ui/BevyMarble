mod components;
mod coords;
mod grid;
pub mod offline;
mod plugin;
mod render;
mod setup;
mod systems;

pub use components::{BigBall, Bullet};
pub use plugin::TerritoryPlugin;
pub use plugin::TerritorySettings;
pub use plugin::CiwsDistanceMetric;
pub use plugin::GameOver;
pub use coords::TERRITORY_LOGIC_WIDTH;
