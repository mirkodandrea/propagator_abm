//! Turning a cursor position into a point on the ground.
//!
//! Deliberately not a mesh raycast. The terrain is 256 chunks and 4.19 M
//! vertices; intersecting that per frame to answer "where is the mouse" would
//! be absurd when the heightfield is right there as an array. Marching the ray
//! and sampling [`scenario::Terrain::height_at`] costs a few dozen bilinear
//! lookups and — importantly — hits the *same* 5 m field everything else is
//! placed on, so a click lands exactly where the crosshair is drawn.
//!
//! The march is coarse-then-bisect: step until the ray passes below the
//! surface, then bisect that interval. Coarse steps can tunnel through a thin
//! ridge seen edge-on, which shows up as a click landing on the valley floor
//! behind it. [`STEP_M`] is sized against the terrain posting to keep that rare.

use bevy::prelude::*;
use scenario::{Pos, Scenario};

/// Coarse march interval, metres along the ray.
const STEP_M: f32 = 12.0;
/// Give up past this: the far plane is 40 km but the window is only 10 km, so
/// a ray aimed at the horizon should fail rather than march forever.
const MAX_DIST_M: f32 = 30_000.0;
/// Bisection steps. 12 halvings take a 12 m bracket under 3 mm.
const REFINE: u32 = 12;

/// Where the cursor is pointing on the ground, in the world frame.
///
/// `None` when the cursor is off-window, or aimed at the sky, or outside the
/// scenario window — all three of which the caller has to handle, because an
/// ignition placed off-map is not a thing.
pub fn cursor_ground(
    scn: &Scenario,
    camera: &Camera,
    cam_tf: &GlobalTransform,
    window: &Window,
) -> Option<Pos> {
    let cursor = cursor_position(camera, window)?;
    let ray = camera.viewport_to_world(cam_tf, cursor)?;
    ray_ground(scn, ray.origin, *ray.direction)
}

/// Cursor position relative to the camera's viewport, in logical pixels.
///
/// [`Window::cursor_position`] is relative to the whole window, whereas
/// [`Camera::viewport_to_world`] and [`Camera::world_to_viewport`] use an
/// origin at the camera viewport's top-left. Those origins differ whenever a
/// docked menu or panel reserves screen space for itself.
pub fn cursor_position(camera: &Camera, window: &Window) -> Option<Vec2> {
    window
        .cursor_position()
        .and_then(|position| window_to_viewport(camera, position))
}

/// Convert a logical position in render-target (window) space to the logical
/// coordinate space used by Bevy's viewport projection helpers.
pub fn window_to_viewport(camera: &Camera, position: Vec2) -> Option<Vec2> {
    relative_to_rect(camera.logical_viewport_rect()?, position)
}

fn relative_to_rect(viewport: Rect, position: Vec2) -> Option<Vec2> {
    viewport
        .contains(position)
        .then_some(position - viewport.min)
}

/// March `dir` from `origin` (both in Bevy space) until it meets the terrain.
pub fn ray_ground(scn: &Scenario, origin: Vec3, dir: Vec3) -> Option<Pos> {
    // Looking up, or level: nothing to hit.
    if dir.y > -1e-4 {
        return None;
    }

    // Skip the empty air above the highest ground instead of marching it: from
    // 14 km out that is most of the ray.
    let ceiling = scn.terrain.elev_max + 10.0;
    let mut t = if origin.y > ceiling {
        (origin.y - ceiling) / -dir.y
    } else {
        0.0
    };

    // Starting below ground happens when the camera is inside a hillside; the
    // caller gets nothing rather than a point behind the viewer.
    if signed_height(scn, origin + dir * t) < 0.0 {
        return None;
    }

    while t < MAX_DIST_M {
        let next = t + STEP_M;
        if signed_height(scn, origin + dir * next) < 0.0 {
            // Bracketed: bisect down to the surface.
            let (mut lo, mut hi) = (t, next);
            for _ in 0..REFINE {
                let mid = (lo + hi) * 0.5;
                if signed_height(scn, origin + dir * mid) < 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let p = crate::frame::to_world(origin + dir * hi);
            return scn.world.contains(p).then_some(p);
        }
        t = next;
    }
    None
}

/// Height of a point above the terrain under it. Negative below ground.
///
/// Outside the window `height_at` clamps to the edge, which would make the
/// march "hit" the ground out at sea; a large positive keeps the ray flying
/// until it either re-enters or runs out.
fn signed_height(scn: &Scenario, v: Vec3) -> f32 {
    let p = crate::frame::to_world(v);
    if !scn.world.contains(p) {
        return f32::MAX;
    }
    v.y - scn.terrain.height_at(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_position_is_made_relative_to_viewport_origin() {
        let viewport = Rect::from_corners(Vec2::new(300.0, 28.0), Vec2::new(1200.0, 800.0));

        assert_eq!(
            relative_to_rect(viewport, Vec2::new(425.0, 103.0)),
            Some(Vec2::new(125.0, 75.0))
        );
        assert_eq!(relative_to_rect(viewport, Vec2::new(299.0, 103.0)), None);
    }
}
