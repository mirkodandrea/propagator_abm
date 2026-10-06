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
    /// Whether the shift turns the fire *onto* the town (valle) or away from it
    /// (borgo, porto). What a forecast-follower needs to know to act on `shift_p`.
    pub shift_threatens: bool,
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
        // Fire in the pines south of the village, wind from the south: straight
        // up the hill at Il Borgo. If the wind backs to the east the fire turns
        // west onto Le Coste. Il Mulino is upwind of both.
        "demo_borgo" => Spec {
            id: "demo_borgo", weather: w(180.0, 35.0, 5.0),
            ignition: Pos { x: 2000.0, y: 1200.0 }, radius_m: 100.0, shift: None,
            climate: Climate { shift_p: (0.2, 0.7), shift_window_s: (12 * 60, 26 * 60), shift_to: w(90.0, 45.0, 5.0), shift_threatens: false },
            duration_s: 60 * 60,
        },
        // A fire on the valley road between two hamlets. The east wind drives it
        // at Casale Ovest; if it swings round to the west, Casale Est is next.
        "demo_valle" => Spec {
            id: "demo_valle", weather: w(90.0, 30.0, 5.0),
            ignition: Pos { x: 2000.0, y: 2000.0 }, radius_m: 100.0,
            shift: Some(WindShift { at_s: 20 * 60, weather: w(270.0, 45.0, 5.0) }),
            climate: Climate { shift_p: (0.25, 0.75), shift_window_s: (12 * 60, 26 * 60), shift_to: w(270.0, 45.0, 5.0), shift_threatens: true },
            duration_s: 60 * 60,
        },
        // Wind from the north, the pines behind the town alight, one road out
        // and it runs through them past La Pineta. The beach does not burn.
        "demo_porto" => Spec {
            id: "demo_porto", weather: w(0.0, 40.0, 5.0),
            ignition: Pos { x: 2300.0, y: 2450.0 }, radius_m: 150.0, shift: None,
            climate: Climate { shift_p: (0.15, 0.6), shift_window_s: (12 * 60, 26 * 60), shift_to: w(45.0, 40.0, 5.0), shift_threatens: false },
            duration_s: 60 * 60,
        },
        _ => return None,
    })
}
