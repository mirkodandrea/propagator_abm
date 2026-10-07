//! The lessons (`docs/demo-spec.md` §1, `docs/demo-spec-gameplay.md` §5),
//! each a test that must *fire* on Rocca Ventosa, drawn seeds, played through
//! `Session` the way a player plays. `table` prints every number used here.
//!
//! Also here: the engine is inert until used (finding 34) and deterministic
//! (gameplay §9: same seed + draw + orders ⇒ same verdict).

use std::sync::OnceLock;

use demo::turn_policy::{self as tp, grid, mean_se, Sel, TurnPolicy, BORGO, COSTE, MULINO};
use demo::*;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

const N: u64 = 40;

fn shifts(seed: u64) -> bool {
    demo::draw(TOWN, seed).unwrap().spec.shift.is_some()
}

fn at(turn: u8, plan: Vec<(TokenId, Sel)>) -> impl Fn(&Session) -> Vec<(TokenId, Sel)> + Send + Sync {
    move |s: &Session| if s.turn().index == turn { plan.clone() } else { vec![] }
}

/// Canadair called at `call`, then one drop a turn on `sel` (or, with `None`,
/// on the district the wind now drives the fire at).
fn canadair(name: &str, call: u8, sel: Option<Sel>) -> TurnPolicy {
    TurnPolicy::new(name.to_string(), move |s: &Session| {
        let k = s.token(TokenId::K);
        let to = sel.unwrap_or(Sel::District(if s.wind_turned() { COSTE } else { BORGO }));
        let kind = match to {
            Sel::District(d) => TargetKind::District(d),
            Sel::Head => TargetKind::Head,
            Sel::Flank(x) => TargetKind::Flank(x),
            Sel::Sky => TargetKind::Sky,
        };
        if s.turn().index == call {
            vec![(TokenId::K, Sel::Sky)]
        } else if k.orderable && k.state != TokenState::NonChiamato && k.doing != Some(kind) {
            vec![(TokenId::K, to)]
        } else {
            vec![]
        }
    })
}

fn policies() -> Vec<TurnPolicy> {
    vec![
        tp::none(),
        tp::patrol_borgo_t1(),
        TurnPolicy::new("patrol-borgo-t3", at(3, vec![(TokenId::P, Sel::District(BORGO))])),
        tp::forecast_patrol_only(),
        tp::patrol_mulino_first(),
        tp::patrol_borgo_coste(),
        TurnPolicy::new("engine-head-t2", at(2, vec![(TokenId::E1, Sel::Head)])),
        TurnPolicy::new("crew-head-t2", at(2, vec![(TokenId::S, Sel::Head)])),
        canadair("canadair-head", 1, Some(Sel::Head)),
        TurnPolicy::new("engines-borgo-t1", at(1, vec![(TokenId::E1, Sel::District(BORGO)), (TokenId::E2, Sel::District(BORGO))])),
        TurnPolicy::new("engines-mulino-t1", at(1, vec![(TokenId::E1, Sel::District(MULINO)), (TokenId::E2, Sel::District(MULINO))])),
        tp::crew_borgo_t1(),
        tp::crew_mulino_t1(),
        canadair("canadair-t1", 1, None),
        canadair("canadair-t3", 3, None),
    ]
}

#[derive(Debug, Clone)]
struct Rec {
    caught: f32,
    homes: f32,
    borgo_homes: f32,
    caught_by: [f32; 3],
    stamps: Vec<Stamp>,
    drops_by_last_turn: u32,
}

fn play(p: &TurnPolicy, seed: u64) -> Rec {
    let mut s = Session::new(&data_dir(), seed).unwrap();
    p.play(&mut s).unwrap();
    let f = s.facts();
    let none = Session::counterfactual_of(&data_dir(), s.draw).unwrap();
    Rec {
        caught: f.families_caught as f32,
        homes: f.homes_hit as f32,
        borgo_homes: f.districts[BORGO].1 as f32,
        caught_by: [f.districts[0].0 as f32, f.districts[1].0 as f32, f.districts[2].0 as f32],
        stamps: s.verdict_against(none).districts.iter().map(|d| d.people).collect(),
        drops_by_last_turn: s.drops().1,
    }
}

/// [seed-1][policy]
fn results() -> &'static Vec<Vec<Rec>> {
    static R: OnceLock<Vec<Vec<Rec>>> = OnceLock::new();
    R.get_or_init(|| {
        let ps = policies();
        let seeds: Vec<u64> = (1..=N).collect();
        let mut out = vec![vec![]; seeds.len()];
        for (seed, _, r) in grid(&seeds, ps.len(), |seed, j| play(&ps[j], seed)) {
            out[(seed - 1) as usize].push(r);
        }
        out
    })
}

fn idx(name: &str) -> usize {
    policies().iter().position(|p| p.name == name).unwrap()
}

/// Mean ± s.e. over seeds, optionally only wind-holds (`Some(false)`) or shift (`Some(true)`).
fn col(j: usize, only: Option<bool>, f: impl Fn(&Rec) -> f32) -> (f32, f32) {
    let v: Vec<f32> = results().iter().enumerate().filter(|(k, _)| only.map_or(true, |w| shifts(*k as u64 + 1) == w)).map(|(_, r)| f(&r[j])).collect();
    mean_se(&v)
}

fn diff(j: usize, k: usize, only: Option<bool>, f: impl Fn(&Rec) -> f32) -> (f32, f32) {
    let v: Vec<f32> = results().iter().enumerate().filter(|(i, _)| only.map_or(true, |w| shifts(*i as u64 + 1) == w)).map(|(_, r)| f(&r[j]) - f(&r[k])).collect();
    mean_se(&v)
}

#[test]
#[ignore]
fn table() {
    println!("\nlessons (N={N}): caught all | holds: Borgo / Coste caught | shifts: Coste caught | Mulino caught | homes all | holds: Borgo homes | Δ homes vs none | drops ≤T+40 | Mulino AllarmeInutile");
    for (j, p) in policies().iter().enumerate() {
        let f = |x: (f32, f32)| format!("{:5.1} ± {:3.1}", x.0, x.1);
        let wolf = results().iter().filter(|r| r[j].stamps[MULINO] == Stamp::AllarmeInutile).count();
        println!(
            "{:34} {} | {} / {} | {} | {} | {} | {} | {} | {} | {wolf}/{N}",
            p.name,
            f(col(j, None, |r| r.caught)),
            f(col(j, Some(false), |r| r.caught_by[BORGO])),
            f(col(j, Some(false), |r| r.caught_by[COSTE])),
            f(col(j, Some(true), |r| r.caught_by[COSTE])),
            f(col(j, None, |r| r.caught_by[MULINO])),
            f(col(j, None, |r| r.homes)),
            f(col(j, Some(false), |r| r.borgo_homes)),
            f(diff(j, 0, None, |r| r.homes)),
            f(col(j, None, |r| r.drops_by_last_turn as f32)),
        );
    }
}

// --- 1. The wind decides who is at risk -----------------------------------------------

#[test]
fn l1_the_wind_decides_who() {
    let none = idx("none");
    let (borgo, _) = col(none, Some(false), |r| r.caught_by[BORGO]);
    let (coste_hold, _) = col(none, Some(false), |r| r.caught_by[COSTE]);
    let (coste_shift, _) = col(none, Some(true), |r| r.caught_by[COSTE]);
    let (mulino, _) = col(none, None, |r| r.caught_by[MULINO]);
    assert!(borgo >= 3.0 * coste_hold.max(1.0), "wind holds: Il Borgo {borgo:.1} vs Le Coste {coste_hold:.1}");
    assert!(coste_shift > 5.0, "the shift should put Le Coste in danger: {coste_shift:.1}");
    assert!(mulino == 0.0, "Il Mulino is upwind in every forecast: {mulino:.1} caught");
}

// --- 2. Warn early ---------------------------------------------------------------------

#[test]
fn l2_warn_early() {
    let (d, se) = diff(idx("patrol-borgo-t3"), idx("patrol-borgo-t1"), Some(false), |r| r.caught_by[BORGO]);
    assert!(d >= 3.0 && d > 3.0 * se, "patrol to Il Borgo at turn 3 catches only {d:.1} ± {se:.1} more families than at turn 1");
}

// --- 3. Don't warn everyone: the patrol is one car -------------------------------------

#[test]
fn l3_a_stop_at_il_mulino_costs_the_districts_at_risk() {
    let (d, se) = diff(idx("patrol-mulino-first"), idx("forecast-patrol"), None, |r| r.caught);
    assert!(d > 2.0 * se && d >= 1.0, "a stop at Il Mulino first catches only {d:.1} ± {se:.1} more families");
    // ... and it is stamped a false alarm, except where the fire really came
    // within the threat distance of Il Mulino (then the stop was justified).
    let j = idx("patrol-mulino-first");
    let mut wolf = 0;
    for (k, r) in results().iter().enumerate() {
        let s = r[j].stamps[MULINO];
        assert!(matches!(s, Stamp::AllarmeInutile | Stamp::InTempo | Stamp::Tardi), "seed {}: {s:?}", k + 1);
        wolf += (s == Stamp::AllarmeInutile) as u64;
    }
    assert!(wolf * 10 >= N * 9, "Il Mulino stamped a false alarm in only {wolf}/{N} sessions");
}

// --- 4. Never attack the head -------------------------------------------------------------

#[test]
fn l4_the_head_saves_nothing() {
    for name in ["engine-head-t2", "crew-head-t2", "canadair-head"] {
        let (d, se) = diff(idx(name), idx("none"), None, |r| r.homes);
        assert!(d.abs() <= 2.0 * se.max(0.5), "{name}: {d:.1} ± {se:.1} homes against no orders");
    }
}

// --- 5. Defend where the fire is going ---------------------------------------------------------

#[test]
fn l5_engines_and_crew_save_homes_only_on_the_fires_path() {
    // When the wind holds the fire goes to Il Borgo.
    let (none, _) = col(idx("none"), Some(false), |r| r.homes);
    let (borgo, _) = col(idx("engines-borgo-t1"), Some(false), |r| r.homes);
    assert!(borgo <= none / 2.0, "two engines on Il Borgo: {borgo:.1} homes hit against {none:.1}");
    let (d, se) = diff(idx("engines-mulino-t1"), idx("none"), None, |r| r.homes);
    assert!(d.abs() <= 2.0 * se.max(0.5), "two engines on Il Mulino change homes hit by {d:.1} ± {se:.1}");
    let (d, se) = diff(idx("crew-borgo-t1"), idx("none"), Some(false), |r| r.homes);
    assert!(d <= -5.0 && -d > 3.0 * se, "the crew on Il Borgo saves only {:.1} ± {se:.1} homes", -d);
    let (d, se) = diff(idx("crew-mulino-t1"), idx("none"), None, |r| r.homes);
    assert!(d.abs() <= 2.0 * se.max(0.5), "the crew on Il Mulino changes homes hit by {d:.1} ± {se:.1}");
}

// --- 6. Call aircraft early -------------------------------------------------------------------

/// Called at turn 1 the plane drops twice before the last turn ends -- except
/// where its own policy breaks off the first run because the flight crosses
/// the fire (it then sits out a turn, `Ritirato`). Called at turn 3 it
/// arrives after the last turn.
#[test]
fn l6_call_the_canadair_early() {
    let (t1, t3) = (idx("canadair-t1"), idx("canadair-t3"));
    let mut two = 0;
    for (k, r) in results().iter().enumerate() {
        two += (r[t1].drops_by_last_turn >= 2) as u64;
        assert!(r[t3].drops_by_last_turn <= 1, "seed {}: called at turn 3, {} drops by T+40", k + 1, r[t3].drops_by_last_turn);
    }
    assert!(two * 10 >= N * 9, "called at turn 1, two drops by T+40 in only {two}/{N} sessions");
    let (d, se) = diff(t1, idx("none"), None, |r| r.homes);
    assert!(d <= -4.0 && -d > 3.0 * se, "the Canadair called at turn 1 saves {:.1} ± {se:.1} homes", -d);
}

// --- 7. People first ------------------------------------------------------------------------

#[test]
fn l7_units_save_homes_warnings_save_people() {
    let none = idx("none");
    let units = idx("engines-borgo-t1");
    let warn = idx("patrol-borgo-t1 + patrol-coste-t2");
    let (uh, uhs) = diff(units, none, None, |r| r.homes);
    let (uc, ucs) = diff(units, none, None, |r| r.caught);
    let (wh, whs) = diff(warn, none, None, |r| r.homes);
    let (wc, wcs) = diff(warn, none, None, |r| r.caught);
    assert!(uh < -3.0 * uhs && uh <= -5.0, "units alone: homes {uh:.1} ± {uhs:.1}");
    assert!(uc.abs() <= 2.0 * ucs.max(0.25), "units alone: families {uc:.1} ± {ucs:.1}");
    assert!(wc < -3.0 * wcs && wc <= -3.0, "warnings alone: families {wc:.1} ± {wcs:.1}");
    assert!(wh.abs() <= 2.0 * whs.max(0.25), "warnings alone: homes {wh:.1} ± {whs:.1}");
}

// --- the engine's own rules (gameplay §9) -------------------------------------------------------

/// Finding 34: a session nobody gives an order in is the counterfactual,
/// exactly -- the patrol, the reinforcement, the crew and the Canadair touch
/// nothing until they are used.
#[test]
fn a_session_without_orders_is_the_counterfactual() {
    for seed in [1, 2, 3] {
        let mut s = Session::new(&data_dir(), seed).unwrap();
        s.finish().unwrap();
        let none = Session::counterfactual_of(&data_dir(), s.draw).unwrap();
        assert_eq!(s.facts(), none, "seed {seed}");
        // The published model (no crew defence, no station) with no orders
        // counts the same families and homes: the variants are inert unused.
        let mut run = Run::with_variant(&data_dir(), s.draw.spec, seed, Variant { defend_homes: true, ..Variant::default() }).unwrap();
        run.play(&[]).unwrap();
        assert_eq!(run.outcome().caught as u32, none.families_caught, "seed {seed}");
        assert_eq!(run.outcome().homes_lost as u32, none.homes_hit, "seed {seed}");
    }
}

/// Same seed + orders ⇒ same verdict, every time (the `play` replay and the
/// kiosk's twin depend on it).
#[test]
fn the_same_orders_give_the_same_verdict() {
    let p = tp::forecast_player();
    let run = || {
        let mut s = Session::new(&data_dir(), 7).unwrap();
        p.play(&mut s).unwrap();
        let none = Session::counterfactual_of(&data_dir(), s.draw).unwrap();
        (s.verdict_against(none), s.log().to_vec(), s.drops())
    };
    assert_eq!(run(), run());
}

/// The reinforcement is not in the tray before turn 3, and is after.
#[test]
fn the_reinforcement_arrives_at_turn_three() {
    let mut s = Session::new(&data_dir(), 1).unwrap();
    assert!(!s.tokens().iter().any(|t| t.id == TokenId::E3));
    assert_eq!(s.assign(TokenId::E3, TargetId(1)), Err(Refusal::NonDisponibile));
    s.end_turn().unwrap();
    s.end_turn().unwrap();
    assert_eq!(s.turn().index, REINFORCEMENT_TURN);
    assert!(s.tokens().iter().any(|t| t.id == TokenId::E3 && t.state == TokenState::Libero));
}

/// The patrol warns a district when it gets there, not when it is sent.
#[test]
fn the_patrol_warns_on_arrival() {
    let mut s = Session::new(&data_dir(), 1).unwrap();
    let borgo = s.target_of(TargetKind::District(BORGO)).unwrap().id;
    let eta = s.preview(TokenId::P, borgo).unwrap().eta_s;
    assert!(eta > 60, "the patrol starts away from Il Borgo ({eta} s)");
    s.assign(TokenId::P, borgo).unwrap();
    let mut warned_at = None;
    s.end_turn_observed(|x| {
        if warned_at.is_none() && x.run.referee.reports[BORGO].warned_at_s.is_some() {
            warned_at = Some(x.time_s());
        }
    })
    .unwrap();
    let w = warned_at.expect("warned within the turn");
    assert!(w >= eta && w < eta + 2 * STEP_S, "warned at {w} s, ETA {eta} s");
    assert!(s.report().lines.iter().any(|l| l.kind == ReportKind::PattugliaArrivata && l.district == Some(BORGO)));
}

/// The Canadair: called, it is in the air 25 minutes later; it can be
/// briefed only during the turn it arrives in; then one drop a turn.
#[test]
fn the_canadair_comes_25_minutes_after_the_call() {
    let mut s = Session::new(&data_dir(), 1).unwrap();
    let sky = s.target_of(TargetKind::Sky).unwrap().id;
    assert!(matches!(s.preview(TokenId::K, sky).unwrap().effect, Effect::Chiamata { turn: 4, .. }));
    s.assign(TokenId::K, sky).unwrap();
    s.end_turn().unwrap();
    assert!(matches!(s.token_state(TokenId::K), TokenState::InArrivo { .. }));
    assert!(!s.token(TokenId::K).orderable, "turn 2: still 17 minutes out");
    s.end_turn().unwrap();
    s.end_turn().unwrap();
    assert_eq!(s.turn().index, 4);
    assert!(s.token(TokenId::K).orderable, "turn 4: overhead within the turn, can be briefed");
}

/// Assigning is refused, with a reason, where the tray says it should be.
#[test]
fn refusals_are_typed() {
    let mut s = Session::new(&data_dir(), 1).unwrap();
    let head = s.target_of(TargetKind::Head).unwrap().id;
    let sky = s.target_of(TargetKind::Sky).unwrap().id;
    let borgo = s.target_of(TargetKind::District(BORGO)).unwrap().id;
    assert_eq!(s.assign(TokenId::P, head), Err(Refusal::BersaglioNonValido));
    assert_eq!(s.assign(TokenId::E1, sky), Err(Refusal::BersaglioNonValido));
    assert_eq!(s.assign(TokenId::E1, TargetId(99)), Err(Refusal::BersaglioSconosciuto));
    s.assign(TokenId::E1, borgo).unwrap();
    assert!(s.unassign(TokenId::E1));
    assert!(!s.unassign(TokenId::E1));
    s.finish().unwrap();
    assert_eq!(s.assign(TokenId::E1, borgo), Err(Refusal::Finita));
}

/// A warning is *in time* only if it lands IN_TIME_MIN before the fire first
/// threatens or reaches the district and it saved somebody against no orders:
/// a warning at T+24 to the district the wind is driving at, on the sessions
/// where it holds, is *Tardi* (on seeds 30, 34, 39 it lands 15-19 minutes
/// before the threat and still saves nobody).
#[test]
fn a_late_warning_is_stamped_late() {
    let seeds: Vec<u64> = (1..=N).filter(|s| !shifts(*s)).collect();
    let r = grid(&seeds, 1, |seed, _| {
        let mut s = Session::new(&data_dir(), seed).unwrap();
        while s.turn().index < 4 {
            s.end_turn().unwrap();
        }
        assert_eq!(s.time_s(), 24 * 60);
        s.run.order(Order::EvacuateDistrict(BORGO));
        s.finish().unwrap();
        let none = Session::counterfactual_of(&data_dir(), s.draw).unwrap();
        s.verdict_against(none).districts[BORGO].people
    });
    for (seed, _, stamp) in r {
        assert_eq!(stamp, Stamp::Tardi, "seed {seed}: a T+24 warning to Il Borgo");
    }
}
