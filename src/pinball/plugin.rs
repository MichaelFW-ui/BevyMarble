use bevy::prelude::*;

use super::layout::spawn_pinball_layout;
use super::systems::*;

pub struct PinballPlugin;

impl Plugin for PinballPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CircleMeshCache>()
            .add_systems(Startup, spawn_pinball_layout)
            .add_systems(
                Startup,
                spawn_initial_marbles.after(spawn_pinball_layout),
            )
            .add_systems(
                Update,
                (
                    check_multiplier_collision,
                    check_action_zone_collision,
                    contain_marbles,
                    assist_stuck_marbles,
                    update_marble_display,
                    sync_marble_text_position,
                ),
            );
    }
}
