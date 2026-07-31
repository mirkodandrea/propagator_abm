//! Running an authored behaviour against a made-up agent.
//!
//! The reason this exists rather than "just run the scenario": a graph is a
//! hypothesis about *when* people act, and the scenario answers that question
//! only for the situations the fire happens to produce. A bench lets the
//! author put the household in the situation they are reasoning about — the
//! order has arrived but the fire is still 2 km away — and read the answer
//! immediately, along with every intermediate value that produced it.
//!
//! Two views of the same evaluation:
//!
//! - a **situation**: one observation in, one traced decision out;
//! - a **sweep**: one field varied across a range, so the author can see
//!   *where* the behaviour changes rather than guessing at thresholds.

use crate::eval::{CompiledGraph, Decision, Trace};
use crate::observation::Observation;
use crate::value::{ActionKind, IntentValue};

/// A named starting situation.
#[derive(Debug, Clone)]
pub struct Situation {
    pub name: &'static str,
    pub note: &'static str,
    pub obs: Observation,
}

/// The situations worth checking every behaviour against.
///
/// Chosen to cover the failures the model has actually produced: a household
/// that never wakes up, one that leaves on an order it should not have
/// believed, one that leaves far too late, and one that drives into a cut road.
pub fn situations() -> Vec<Situation> {
    let base = Observation::default();

    let quiet = Observation { time_min: 0.0, ..base };

    let smoke = Observation {
        time_min: 12.0,
        fire_distance_m: 1400.0,
        cue: 0.18,
        threat: 0.02,
        ..base
    };

    let ordered = Observation {
        time_min: 20.0,
        fire_distance_m: 1100.0,
        cue: 0.24,
        threat: 0.05,
        order_issued: true,
        warning_received: true,
        minutes_since_order: 4.0,
        ..base
    };

    let close = Observation {
        time_min: 55.0,
        fire_distance_m: 250.0,
        cue: 0.62,
        threat: 0.28,
        radiant: 0.2,
        ember: 0.35,
        order_issued: true,
        warning_received: true,
        minutes_since_order: 38.0,
        ..base
    };

    let at_the_fence = Observation {
        time_min: 70.0,
        fire_distance_m: 40.0,
        cue: 0.9,
        threat: 0.72,
        radiant: 0.8,
        ember: 0.7,
        structure_alight: false,
        order_issued: true,
        warning_received: true,
        minutes_since_order: 52.0,
        ..base
    };

    let cut_off = Observation {
        route_blocked: true,
        refuge_distance_m: f32::INFINITY,
        ..at_the_fence
    };

    let defender = Observation { intent: IntentValue::StayDefend, defensible_space: 0.8, ..close };

    let unwarned = Observation {
        trust_authority: 0.15,
        warning_received: true,
        order_issued: true,
        ..ordered
    };

    vec![
        Situation { name: "Quiet", note: "Nothing has happened yet.", obs: quiet },
        Situation {
            name: "Smoke on the ridge",
            note: "A column visible 1.4 km away. No order, no heat.",
            obs: smoke,
        },
        Situation {
            name: "Order given, fire distant",
            note: "The order has arrived over their channel; the fire is still 1.1 km off.",
            obs: ordered,
        },
        Situation {
            name: "Order given, low trust",
            note: "Same, but a household that does not act on official instructions.",
            obs: unwarned,
        },
        Situation {
            name: "Fire 250 m out",
            note: "Embers landing, order 38 minutes old. The last comfortable moment to drive.",
            obs: close,
        },
        Situation {
            name: "Defender, fire 250 m out",
            note: "The same moment for a household that meant to stay.",
            obs: defender,
        },
        Situation {
            name: "Fire at the fence",
            note: "Not survivable outside. Driving out is still possible.",
            obs: at_the_fence,
        },
        Situation {
            name: "Cut off",
            note: "The same, with every route to a refuge burnt. Sheltering has to win here.",
            obs: cut_off,
        },
    ]
}

/// Which observation field a sweep varies.
///
/// A closed set rather than a string, so the editor can only ever offer a
/// sweep that will actually do something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepField {
    TimeMin,
    Threat,
    FireDistanceM,
    Cue,
    RiskPerception,
    TrustAuthority,
    MinutesSinceOrder,
    Jitter,
}

impl SweepField {
    pub const ALL: [SweepField; 8] = [
        SweepField::TimeMin,
        SweepField::Threat,
        SweepField::FireDistanceM,
        SweepField::Cue,
        SweepField::RiskPerception,
        SweepField::TrustAuthority,
        SweepField::MinutesSinceOrder,
        SweepField::Jitter,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SweepField::TimeMin => "Time (min)",
            SweepField::Threat => "Threat at home",
            SweepField::FireDistanceM => "Distance to fire (m)",
            SweepField::Cue => "Perceived alarm",
            SweepField::RiskPerception => "Risk perception",
            SweepField::TrustAuthority => "Trust in authority",
            SweepField::MinutesSinceOrder => "Minutes since order",
            SweepField::Jitter => "Individual variation",
        }
    }

    /// The range worth sweeping, low to high.
    pub fn range(self) -> (f32, f32) {
        match self {
            SweepField::TimeMin => (0.0, 120.0),
            SweepField::FireDistanceM => (0.0, 2500.0),
            SweepField::MinutesSinceOrder => (0.0, 120.0),
            _ => (0.0, 1.0),
        }
    }

    pub fn set(self, obs: &mut Observation, v: f32) {
        match self {
            SweepField::TimeMin => obs.time_min = v,
            SweepField::Threat => obs.threat = v,
            SweepField::FireDistanceM => obs.fire_distance_m = v,
            SweepField::Cue => obs.cue = v,
            SweepField::RiskPerception => obs.risk_perception = v,
            SweepField::TrustAuthority => obs.trust_authority = v,
            SweepField::MinutesSinceOrder => obs.minutes_since_order = v,
            SweepField::Jitter => obs.jitter = v,
        }
    }

    pub fn get(self, obs: &Observation) -> f32 {
        match self {
            SweepField::TimeMin => obs.time_min,
            SweepField::Threat => obs.threat,
            SweepField::FireDistanceM => obs.fire_distance_m,
            SweepField::Cue => obs.cue,
            SweepField::RiskPerception => obs.risk_perception,
            SweepField::TrustAuthority => obs.trust_authority,
            SweepField::MinutesSinceOrder => obs.minutes_since_order,
            SweepField::Jitter => obs.jitter,
        }
    }
}

/// One point of a sweep.
#[derive(Debug, Clone, Copy)]
pub struct SweepPoint {
    pub x: f32,
    pub action: ActionKind,
    pub priority: f32,
    pub urgency: f32,
}

/// Evaluate `graph` across `field`, holding everything else at `base`.
pub fn sweep(
    graph: &CompiledGraph,
    base: &Observation,
    field: SweepField,
    steps: usize,
) -> Vec<SweepPoint> {
    let (lo, hi) = field.range();
    let steps = steps.max(2);
    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps - 1) as f32;
            let x = lo + (hi - lo) * t;
            let mut obs = *base;
            field.set(&mut obs, x);
            let d = graph.eval(&obs);
            SweepPoint { x, action: d.action, priority: d.priority, urgency: d.urgency }
        })
        .collect()
}

/// The boundaries in a sweep — where the action the behaviour produces
/// changes. This is the answer the author is usually after, and reading it off
/// a strip of colour is guesswork.
pub fn transitions(points: &[SweepPoint]) -> Vec<(f32, ActionKind, ActionKind)> {
    points
        .windows(2)
        .filter(|w| w[0].action != w[1].action)
        .map(|w| (w[1].x, w[0].action, w[1].action))
        .collect()
}

/// One subtype's answer to one situation, for the comparison table.
#[derive(Debug, Clone)]
pub struct Answer {
    pub subtype: String,
    pub decision: Decision,
    pub trace: Trace,
}

/// Run several compiled subtypes against one situation.
pub fn compare_subtypes(
    graphs: &[(String, &CompiledGraph)],
    obs: &Observation,
) -> Vec<Answer> {
    graphs
        .iter()
        .map(|(id, g)| {
            let (decision, trace) = g.eval_traced(obs);
            Answer { subtype: id.clone(), decision, trace }
        })
        .collect()
}
