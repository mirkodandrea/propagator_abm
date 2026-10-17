//! The tactical view: while the world is paused for orders, the scenery that
//! carries no decision (ground, plants, sea) is drawn toward grey, so houses,
//! units, fire and the interface stand out. The amount eases in and out; the
//! shaders only read it (`retro.wgsl` for terrain and foliage, `water.wgsl`).

use bevy::prelude::*;

use crate::kiosk::{Kiosk, Phase};
use crate::retro::RetroMaterial;
use crate::sea::WaterMaterial;
use crate::sim::Sim;

/// Desaturation at full effect. `KIOSK_TACTICAL=0..1` overrides it (0 = off).
const DEFAULT_AMOUNT: f32 = 0.75;
/// Real seconds to ease fully in or out.
const EASE_S: f32 = 0.4;

#[derive(Resource)]
pub struct Tactical {
    /// Current amount, 0..1 of `max`.
    pub level: f32,
    max: f32,
}

pub struct TacticalPlugin;

impl Plugin for TacticalPlugin {
    fn build(&self, app: &mut App) {
        let max = std::env::var("KIOSK_TACTICAL")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map_or(DEFAULT_AMOUNT, |v| v.clamp(0.0, 1.0));
        app.insert_resource(Tactical { level: 0.0, max })
            .add_systems(Update, apply);
    }
}

fn apply(
    time: Res<Time>,
    kiosk: Option<Res<Kiosk>>,
    sim: Option<Res<Sim>>,
    mut tactical: ResMut<Tactical>,
    mut retro: ResMut<Assets<RetroMaterial>>,
    mut water: ResMut<Assets<WaterMaterial>>,
) {
    let (Some(kiosk), Some(sim)) = (kiosk, sim) else { return };
    let playing = matches!(kiosk.phase, Phase::Tutorial | Phase::Giorno2 | Phase::Gioco);
    let want = if playing && sim.paused() { 1.0 } else { 0.0 };
    let step = time.delta_seconds() / EASE_S;
    let next = if want > tactical.level { (tactical.level + step).min(want) } else { (tactical.level - step).max(want) };
    if next == tactical.level {
        return;
    }
    tactical.level = next;
    // smoothstep, so the ease has no visible start or stop
    let amount = next * next * (3.0 - 2.0 * next) * tactical.max;
    // only the terrain (w = 1) and foliage (w = 2) materials read it
    for (_, m) in retro.iter_mut() {
        if (1.0..2.5).contains(&m.extension.enabled.w) {
            m.extension.tactical.x = amount;
        }
    }
    for (_, m) in water.iter_mut() {
        m.uniform.tactical.x = amount;
    }
}
