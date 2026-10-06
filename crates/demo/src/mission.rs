//! What each demo town opens with.
//! Pinned from `tests/sizing.rs` sweeps, per
//! finding 38: a tuned constant is scenario-specific.

use fire::Weather;
use scenario::Pos;

/// A scripted change of wind: the whole point of `demo_valle`.
#[derive(Debug, Clone, Copy)]
pub struct WindShift {
    pub at_s: i64,
    pub weather: Weather,
}

/// The odds a town's weather is drawn from.
#[derive(Debug, Clone, Copy)]
pub struct Climate {
    /// Per-session chance of a wind shift is drawn uniformly from this range;
    /// the forecast reports it, the draw then rolls against it.
    pub shift_p: (f32, f32),
    /// Earliest and latest simulated second a shift can arrive.
    pub shift_window_s: (i64, i64),
    pub shift_to: Weather,
}

#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub id: &'static str,
    pub weather: Weather,
    /// Where the opening fire is lit, in the world frame (metres).
    pub ignition: Pos,
    pub radius_m: f32,
    pub shift: Option<WindShift>,
    /// What a *drawn* session may do (spec 6.1): the chance range of a wind
    /// shift, when it can arrive, and what the wind becomes. `shift` above is the
    /// pinned script the beat tests use; a draw replaces it.
    pub climate: Climate,
    /// Simulated length of the mission. ~3 min real time at the demo speed.
    pub duration_s: i64,
}

pub const ALL: [&str; 3] = ["demo_borgo", "demo_valle", "demo_porto"];

pub fn spec(id: &str) -> Option<Spec> {
    let w = |dir, kmh, m| Weather { wind_dir_deg: dir, wind_speed_kmh: kmh, moisture_pct: m };
    Some(match id {
        // Fire from the south, wind from the south: straight up the hill at the
        // village. A 60 m start (~1 ha; spec 6.4). Five-seed sweep (tests/small.rs):
        // 38 families caught at home with no order, 14 / 21 / 37 for an order at
        // T+0 / T+15 / T+30.
        "demo_borgo" => Spec {
            id: "demo_borgo", weather: w(180.0, 35.0, 7.0),
            ignition: Pos { x: 2050.0, y: 1300.0 }, radius_m: 60.0, shift: None,
            // A shift to an east wind drives the fire away from the village.
            climate: Climate { shift_p: (0.15, 0.85), shift_window_s: (15 * 60, 40 * 60), shift_to: w(90.0, 35.0, 7.0) },
            duration_s: 75 * 60,
        },
        // Lit on the valley floor between two hamlets. The east wind drives it
        // at hamlet A, which sees it coming and leaves; at T+30 the wind goes
        // round to the west and hamlet B, which had no reason to worry, is next.
        "demo_valle" => Spec {
            id: "demo_valle", weather: w(90.0, 25.0, 6.0),
            ignition: Pos { x: 2200.0, y: 2400.0 }, radius_m: 60.0,
            shift: Some(WindShift { at_s: 30 * 60, weather: w(270.0, 45.0, 6.0) }),
            climate: Climate { shift_p: (0.6, 0.95), shift_window_s: (20 * 60, 40 * 60), shift_to: w(270.0, 45.0, 6.0) },
            duration_s: 90 * 60,
        },
        // Wind from the north, the pines behind the town alight, one road out
        // and it runs through them.
        "demo_porto" => Spec {
            id: "demo_porto", weather: w(0.0, 35.0, 7.0),
            ignition: Pos { x: 2000.0, y: 2000.0 }, radius_m: 60.0, shift: None,
            climate: Climate { shift_p: (0.15, 0.85), shift_window_s: (15 * 60, 40 * 60), shift_to: w(90.0, 35.0, 7.0) },
            duration_s: 75 * 60,
        },
        _ => return None,
    })
}
