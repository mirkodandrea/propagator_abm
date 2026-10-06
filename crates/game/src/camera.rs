//! The orbit camera's state. The kiosk drives it (`kiosk::view`).

use bevy::prelude::*;

#[derive(Component)]
pub struct OrbitCamera {
    /// Point on the ground the camera looks at, in Bevy space.
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { focus: Vec3::ZERO, distance: 3500.0, yaw: 0.35, pitch: -0.75 }
    }
}
