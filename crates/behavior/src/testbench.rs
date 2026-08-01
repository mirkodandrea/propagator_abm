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

use crate::domain::Domain;
use crate::eval::{CompiledGraph, Decision, Trace};
use crate::observation::{HouseholdObs, Observation, PersonObs, UnitObs};
use crate::value::{ActionKind, IntentValue, UnitKindKey};

/// A named starting situation.
#[derive(Debug, Clone)]
pub struct Situation {
    pub name: &'static str,
    pub note: &'static str,
    pub obs: Observation,
}

/// The situations worth checking a behaviour of `domain` against.
pub fn situations_for(domain: Domain) -> Vec<Situation> {
    match domain {
        Domain::Household => situations(),
        Domain::SuppressionUnit => unit_situations(),
        Domain::Person => person_situations(),
    }
}

/// The situations worth checking every civilian behaviour against.
///
/// Chosen to cover the failures the model has actually produced: a household
/// that never wakes up, one that leaves on an order it should not have
/// believed, one that leaves far too late, and one that drives into a cut road.
pub fn situations() -> Vec<Situation> {
    let base = HouseholdObs::default();

    let quiet = HouseholdObs { time_min: 0.0, ..base };

    let smoke = HouseholdObs {
        time_min: 12.0,
        fire_distance_m: 1400.0,
        cue: 0.18,
        threat: 0.02,
        ..base
    };

    let ordered = HouseholdObs {
        time_min: 20.0,
        fire_distance_m: 1100.0,
        cue: 0.24,
        threat: 0.05,
        order_issued: true,
        warning_received: true,
        minutes_since_order: 4.0,
        ..base
    };

    let close = HouseholdObs {
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

    let at_the_fence = HouseholdObs {
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

    let cut_off = HouseholdObs {
        route_blocked: true,
        refuge_distance_m: f32::INFINITY,
        ..at_the_fence
    };

    let defender = HouseholdObs { intent: IntentValue::StayDefend, defensible_space: 0.8, ..close };

    let unwarned = HouseholdObs {
        trust_authority: 0.15,
        warning_received: true,
        order_issued: true,
        ..ordered
    };

    vec![
        Situation { name: "Quiet", note: "Nothing has happened yet.", obs: quiet.into() },
        Situation {
            name: "Smoke on the ridge",
            note: "A column visible 1.4 km away. No order, no heat.",
            obs: smoke.into(),
        },
        Situation {
            name: "Order given, fire distant",
            note: "The order has arrived over their channel; the fire is still 1.1 km off.",
            obs: ordered.into(),
        },
        Situation {
            name: "Order given, low trust",
            note: "Same, but a household that does not act on official instructions.",
            obs: unwarned.into(),
        },
        Situation {
            name: "Fire 250 m out",
            note: "Embers landing, order 38 minutes old. The last comfortable moment to drive.",
            obs: close.into(),
        },
        Situation {
            name: "Defender, fire 250 m out",
            note: "The same moment for a household that meant to stay.",
            obs: defender.into(),
        },
        Situation {
            name: "Fire at the fence",
            note: "Not survivable outside. Driving out is still possible.",
            obs: at_the_fence.into(),
        },
        Situation {
            name: "Cut off",
            note: "The same, with every route to a refuge burnt. Sheltering has to win here.",
            obs: cut_off.into(),
        },
    ]
}

/// The situations worth checking every unit policy against.
///
/// Chosen the same way as the civilian ones: each is a moment the hand-written
/// policy either handled or visibly did not. The last two are the second kind —
/// an engine stranded short of its order and a crew standing in the black are
/// both things the shipped model has no answer for beyond a note in the panel.
pub fn unit_situations() -> Vec<Situation> {
    let engine = UnitObs {
        kind: UnitKindKey::Engine,
        carries_water: true,
        water_fraction: 1.0,
        has_task: true,
        task_reachable: true,
        work_available: true,
        distance_to_base_m: 2200.0,
        ..UnitObs::default()
    };

    let staged = UnitObs {
        has_task: false,
        work_available: false,
        distance_to_task_m: 1.0e6,
        distance_to_base_m: 0.0,
        ..engine
    };

    let responding = UnitObs {
        time_min: 8.0,
        is_moving: true,
        distance_to_task_m: 1600.0,
        distance_to_fire_m: 1800.0,
        minutes_on_task: 3.0,
        ..engine
    };

    let working = UnitObs {
        time_min: 22.0,
        is_working: true,
        distance_to_task_m: 10.0,
        distance_to_fire_m: 90.0,
        threat_here: 0.12,
        water_fraction: 0.55,
        minutes_on_task: 14.0,
        ..engine
    };

    let dry = UnitObs { water_fraction: 0.0, ..working };

    let heating_up = UnitObs {
        threat_here: 0.30,
        heat_fraction: 0.45,
        distance_to_fire_m: 45.0,
        ..working
    };

    let over_the_limit = UnitObs {
        threat_here: 0.48,
        heat_fraction: 0.25,
        distance_to_fire_m: 20.0,
        ..working
    };

    let stranded = UnitObs {
        time_min: 30.0,
        task_reachable: false,
        is_working: true,
        work_available: false,
        distance_to_task_m: 620.0,
        minutes_on_task: 12.0,
        ..engine
    };

    let crew_in_the_black = UnitObs {
        kind: UnitKindKey::HandCrew,
        carries_water: false,
        water_fraction: 0.0,
        time_min: 65.0,
        is_working: true,
        work_available: false,
        line_progress: 1.0,
        distance_to_fire_m: 400.0,
        minutes_on_task: 48.0,
        ..engine
    };

    let tanker_over_the_front = UnitObs {
        kind: UnitKindKey::AirTanker,
        carries_water: true,
        water_fraction: 1.0,
        time_min: 40.0,
        is_working: true,
        threat_here: 0.85,
        distance_to_fire_m: 0.0,
        minutes_on_task: 2.0,
        ..engine
    };

    vec![
        Situation {
            name: "Staged",
            note: "At the assembly point with no order. Nothing should fire here.",
            obs: staged.into(),
        },
        Situation {
            name: "Responding",
            note: "An engine 1.6 km from where it was sent, tank full. Carrying on is the answer.",
            obs: responding.into(),
        },
        Situation {
            name: "Working the fuel",
            note: "On scene, pumping, half a tank left and the front 90 m off.",
            obs: working.into(),
        },
        Situation {
            name: "Tank dry",
            note: "The same engine with nothing left to pump. It has to go for water.",
            obs: dry.into(),
        },
        Situation {
            name: "Taking heat",
            note: "Below the threat limit, but it has been absorbing exposure for a while. \
                   The shipped policy stays; a cautious one leaves.",
            obs: heating_up.into(),
        },
        Situation {
            name: "Past the safety limit",
            note: "0.48 of threat with the front 20 m away. Withdrawing has to win here, \
                   whatever the order was.",
            obs: over_the_limit.into(),
        },
        Situation {
            name: "Stranded short of the order",
            note: "The tarmac ended 620 m out and there is nothing in reach to wet. The \
                   shipped policy has no answer beyond a note in the panel.",
            obs: stranded.into(),
        },
        Situation {
            name: "Crew in the black",
            note: "Line complete, the front long past, forty-eight minutes on the same order. \
                   A crew that could be somewhere else.",
            obs: crew_in_the_black.into(),
        },
        Situation {
            name: "Tanker over the front",
            note: "0.85 of threat, which would pull any ground unit out — and must not pull \
                   out an aircraft. This is what the per-kind limits are for.",
            obs: tanker_over_the_front.into(),
        },
    ]
}

/// The situations worth checking a separated person's behaviour against.
///
/// Eight, and every one of them is a moment the model previously had no answer
/// for at all: before this domain existed, someone who was out walked to a
/// refuge and nothing they saw on the way changed that.
pub fn person_situations() -> Vec<Situation> {
    let base = PersonObs::default();

    let out_and_about = PersonObs { time_min: 0.0, is_moving: false, ..base };

    let walking = PersonObs {
        time_min: 8.0,
        fire_distance_m: 1600.0,
        cue: 0.15,
        refuge_distance_m: 500.0,
        home_distance_m: 900.0,
        ..base
    };

    let nearly_home = PersonObs {
        time_min: 10.0,
        fire_distance_m: 1300.0,
        cue: 0.22,
        home_distance_m: 200.0,
        refuge_distance_m: 800.0,
        ..base
    };

    let family_gone = PersonObs {
        time_min: 35.0,
        fire_distance_m: 800.0,
        cue: 0.4,
        home_distance_m: 300.0,
        refuge_distance_m: 700.0,
        household_at_home: false,
        household_safe: true,
        ..base
    };

    let house_burning = PersonObs {
        time_min: 55.0,
        fire_distance_m: 200.0,
        cue: 0.7,
        threat: 0.2,
        home_distance_m: 400.0,
        home_alight: true,
        ..base
    };

    let smoke_across_the_road = PersonObs {
        time_min: 45.0,
        fire_distance_m: 150.0,
        cue: 0.6,
        threat: 0.45,
        heat_fraction: 0.2,
        refuge_distance_m: 1200.0,
        ..base
    };

    let cut_off = PersonObs {
        time_min: 62.0,
        fire_distance_m: 60.0,
        cue: 0.9,
        threat: 0.75,
        heat_fraction: 0.4,
        refuge_distance_m: f32::INFINITY,
        home_distance_m: f32::INFINITY,
        route_blocked: true,
        ..base
    };

    let slow_and_alone = PersonObs {
        time_min: 25.0,
        fire_distance_m: 900.0,
        cue: 0.35,
        age: 81.0,
        walk_speed: 0.6,
        needs_assistance: true,
        refuge_distance_m: 1400.0,
        home_distance_m: 350.0,
        ..base
    };

    vec![
        Situation {
            name: "Out and about",
            note: "Away from home, has not started moving, and nothing has happened yet. \
                   The first tick: whatever this produces is what everyone who was out \
                   does at T+0.",
            obs: out_and_about.into(),
        },
        Situation {
            name: "Walking out",
            note: "On the way to a refuge with the fire well away. The common case, and \
                   the one where a behaviour should be doing nothing.",
            obs: walking.into(),
        },
        Situation {
            name: "Nearly home",
            note: "Home is 200 m away and the refuge is 800 m. If reunification is ever \
                   going to fire, it fires here — and this is the case where it is \
                   arguably the right call.",
            obs: nearly_home.into(),
        },
        Situation {
            name: "Family already out",
            note: "Home is close and the house is empty: the family left without them. \
                   Going back now buys nothing at all, and whether the behaviour knows \
                   that is the whole question.",
            obs: family_gone.into(),
        },
        Situation {
            name: "House alight",
            note: "There is nothing to go home to. A branch that still sends them is \
                   modelling someone who does not know, which is realistic and lethal.",
            obs: house_burning.into(),
        },
        Situation {
            name: "Smoke across the road",
            note: "Threat 0.45: unpleasant, survivable, and a long way still to walk. \
                   Where the survivability limit sits decides whether they push through.",
            obs: smoke_across_the_road.into(),
        },
        Situation {
            name: "Cut off",
            note: "Nowhere left to walk, in the open, with heat accumulating. Shelter has \
                   to win here or the behaviour walks them into the fire.",
            obs: cut_off.into(),
        },
        Situation {
            name: "Slow and alone",
            note: "Eighty-one, needs help, 1.4 km from the refuge at 0.6 m/s: over half an \
                   hour of walking. The profile that shows whether a warning timing works \
                   for the people it has to work for.",
            obs: slow_and_alone.into(),
        },
    ]
}

/// Which observation field a sweep varies.
///
/// A closed set rather than a string, so the editor can only ever offer a
/// sweep that will actually do something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepField {
    // households
    TimeMin,
    Threat,
    FireDistanceM,
    Cue,
    RiskPerception,
    TrustAuthority,
    MinutesSinceOrder,
    Jitter,
    // suppression units
    UnitThreat,
    UnitHeat,
    UnitWater,
    UnitDistanceToFire,
    UnitDistanceToTask,
    UnitMinutesOnTask,
    UnitLineProgress,
    UnitJitter,
    // separated people
    PersonThreat,
    PersonHeat,
    PersonCue,
    PersonFireDistance,
    PersonRefugeDistance,
    PersonHomeDistance,
    PersonAge,
    PersonJitter,
}

impl SweepField {
    const HOUSEHOLD: [SweepField; 8] = [
        SweepField::TimeMin,
        SweepField::Threat,
        SweepField::FireDistanceM,
        SweepField::Cue,
        SweepField::RiskPerception,
        SweepField::TrustAuthority,
        SweepField::MinutesSinceOrder,
        SweepField::Jitter,
    ];

    const UNIT: [SweepField; 8] = [
        SweepField::UnitThreat,
        SweepField::UnitHeat,
        SweepField::UnitWater,
        SweepField::UnitDistanceToFire,
        SweepField::UnitDistanceToTask,
        SweepField::UnitMinutesOnTask,
        SweepField::UnitLineProgress,
        SweepField::UnitJitter,
    ];

    const PERSON: [SweepField; 8] = [
        SweepField::PersonThreat,
        SweepField::PersonHeat,
        SweepField::PersonCue,
        SweepField::PersonFireDistance,
        SweepField::PersonRefugeDistance,
        SweepField::PersonHomeDistance,
        SweepField::PersonAge,
        SweepField::PersonJitter,
    ];

    /// The fields a graph of `domain` can be swept over. Offering the others
    /// would offer a sweep that provably does nothing.
    pub fn for_domain(d: Domain) -> &'static [SweepField] {
        match d {
            Domain::Household => &SweepField::HOUSEHOLD,
            Domain::SuppressionUnit => &SweepField::UNIT,
            Domain::Person => &SweepField::PERSON,
        }
    }

    pub fn domain(self) -> Domain {
        if SweepField::UNIT.contains(&self) {
            Domain::SuppressionUnit
        } else if SweepField::PERSON.contains(&self) {
            Domain::Person
        } else {
            Domain::Household
        }
    }

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
            SweepField::UnitThreat => "Threat at the unit",
            SweepField::UnitHeat => "Accumulated heat",
            SweepField::UnitWater => "Water remaining",
            SweepField::UnitDistanceToFire => "Distance to fire (m)",
            SweepField::UnitDistanceToTask => "Distance to the order (m)",
            SweepField::UnitMinutesOnTask => "Minutes on this order",
            SweepField::UnitLineProgress => "Line cut so far",
            SweepField::UnitJitter => "Individual variation",
            SweepField::PersonThreat => "Threat where they stand",
            SweepField::PersonHeat => "Accumulated heat",
            SweepField::PersonCue => "Perceived alarm",
            SweepField::PersonFireDistance => "Distance to fire (m)",
            SweepField::PersonRefugeDistance => "Distance to refuge (m)",
            SweepField::PersonHomeDistance => "Distance home (m)",
            SweepField::PersonAge => "Age",
            SweepField::PersonJitter => "Individual variation",
        }
    }

    /// The range worth sweeping, low to high.
    pub fn range(self) -> (f32, f32) {
        match self {
            SweepField::TimeMin => (0.0, 120.0),
            SweepField::FireDistanceM => (0.0, 2500.0),
            SweepField::MinutesSinceOrder => (0.0, 120.0),
            SweepField::UnitDistanceToFire => (0.0, 2000.0),
            SweepField::UnitDistanceToTask => (0.0, 3000.0),
            SweepField::UnitMinutesOnTask => (0.0, 120.0),
            SweepField::PersonFireDistance => (0.0, 2500.0),
            SweepField::PersonRefugeDistance => (0.0, 3000.0),
            SweepField::PersonHomeDistance => (0.0, 3000.0),
            SweepField::PersonAge => (0.0, 100.0),
            _ => (0.0, 1.0),
        }
    }

    /// Set the field this sweep varies.
    ///
    /// A no-op when `obs` is of the other domain, which the editor's field list
    /// already prevents: a sweep that silently varied nothing would look
    /// exactly like a behaviour with no threshold in it.
    pub fn set(self, obs: &mut Observation, v: f32) {
        match self {
            SweepField::TimeMin => with_h(obs, |h| h.time_min = v),
            SweepField::Threat => with_h(obs, |h| h.threat = v),
            SweepField::FireDistanceM => with_h(obs, |h| h.fire_distance_m = v),
            SweepField::Cue => with_h(obs, |h| h.cue = v),
            SweepField::RiskPerception => with_h(obs, |h| h.risk_perception = v),
            SweepField::TrustAuthority => with_h(obs, |h| h.trust_authority = v),
            SweepField::MinutesSinceOrder => with_h(obs, |h| h.minutes_since_order = v),
            SweepField::Jitter => with_h(obs, |h| h.jitter = v),
            SweepField::UnitThreat => with_u(obs, |u| u.threat_here = v),
            SweepField::UnitHeat => with_u(obs, |u| u.heat_fraction = v),
            SweepField::UnitWater => with_u(obs, |u| u.water_fraction = v),
            SweepField::UnitDistanceToFire => with_u(obs, |u| u.distance_to_fire_m = v),
            SweepField::UnitDistanceToTask => with_u(obs, |u| u.distance_to_task_m = v),
            SweepField::UnitMinutesOnTask => with_u(obs, |u| u.minutes_on_task = v),
            SweepField::UnitLineProgress => with_u(obs, |u| u.line_progress = v),
            SweepField::UnitJitter => with_u(obs, |u| u.jitter = v),
            SweepField::PersonThreat => with_p(obs, |p| p.threat = v),
            SweepField::PersonHeat => with_p(obs, |p| p.heat_fraction = v),
            SweepField::PersonCue => with_p(obs, |p| p.cue = v),
            SweepField::PersonFireDistance => with_p(obs, |p| p.fire_distance_m = v),
            SweepField::PersonRefugeDistance => with_p(obs, |p| p.refuge_distance_m = v),
            SweepField::PersonHomeDistance => with_p(obs, |p| p.home_distance_m = v),
            SweepField::PersonAge => with_p(obs, |p| p.age = v),
            SweepField::PersonJitter => with_p(obs, |p| p.jitter = v),
        }
    }

    pub fn get(self, obs: &Observation) -> f32 {
        let (h, u, p) = (obs.household(), obs.unit(), obs.person());
        match self {
            SweepField::TimeMin => h.time_min,
            SweepField::Threat => h.threat,
            SweepField::FireDistanceM => h.fire_distance_m,
            SweepField::Cue => h.cue,
            SweepField::RiskPerception => h.risk_perception,
            SweepField::TrustAuthority => h.trust_authority,
            SweepField::MinutesSinceOrder => h.minutes_since_order,
            SweepField::Jitter => h.jitter,
            SweepField::UnitThreat => u.threat_here,
            SweepField::UnitHeat => u.heat_fraction,
            SweepField::UnitWater => u.water_fraction,
            SweepField::UnitDistanceToFire => u.distance_to_fire_m,
            SweepField::UnitDistanceToTask => u.distance_to_task_m,
            SweepField::UnitMinutesOnTask => u.minutes_on_task,
            SweepField::UnitLineProgress => u.line_progress,
            SweepField::UnitJitter => u.jitter,
            SweepField::PersonThreat => p.threat,
            SweepField::PersonHeat => p.heat_fraction,
            SweepField::PersonCue => p.cue,
            SweepField::PersonFireDistance => p.fire_distance_m,
            SweepField::PersonRefugeDistance => p.refuge_distance_m,
            SweepField::PersonHomeDistance => p.home_distance_m,
            SweepField::PersonAge => p.age,
            SweepField::PersonJitter => p.jitter,
        }
    }
}

fn with_h(obs: &mut Observation, f: impl FnOnce(&mut HouseholdObs)) {
    if let Some(h) = obs.household_mut() {
        f(h);
    }
}

fn with_u(obs: &mut Observation, f: impl FnOnce(&mut UnitObs)) {
    if let Some(u) = obs.unit_mut() {
        f(u);
    }
}

fn with_p(obs: &mut Observation, f: impl FnOnce(&mut PersonObs)) {
    if let Some(p) = obs.person_mut() {
        f(p);
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
