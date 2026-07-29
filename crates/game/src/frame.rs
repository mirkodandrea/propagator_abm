//! Mapping the scenario's world frame onto Bevy's.
//!
//! The scenario frame is a map frame: +x east, +y north, metres, origin at the
//! window's south-west corner. Bevy is Y-up with -Z forward, so north becomes
//! -Z and elevation becomes +Y. Every conversion goes through here so the sign
//! flip lives in exactly one place.

use bevy::prelude::*;
use scenario::Pos;

/// Scenario world metres + elevation -> Bevy translation.
#[inline]
pub fn to_bevy(p: Pos, elev: f32) -> Vec3 {
    Vec3::new(p.x, elev, -p.y)
}

/// Bevy translation -> scenario world metres (elevation dropped).
#[inline]
pub fn to_world(v: Vec3) -> Pos {
    Pos { x: v.x, y: -v.z }
}
