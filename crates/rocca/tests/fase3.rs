//! Phase 3 checks (03-PIANO-DI-AZIONE, fase 3; CLAUDE.md «Verifiche») on the
//! game's territory, `data/scenarios/rocca_ventosa`.

use std::path::{Path, PathBuf};

use rocca::{Civil, Game, Outcome, Plan, Verdict};
use scenario::population::Status;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// The effects of the orders, under the rules before 2026-10-13 (any
/// evacuation at any time); the mayor's rule has its own tests below.
fn game(case: &str) -> Game {
    let mut g = Game::new(&data(), case, 1).unwrap();
    g.mayor = None;
    g
}

fn idx(g: &Game, name: &str) -> usize {
    g.district_index(name).unwrap()
}

fn run(case: &str, prio: &[&str], civil: &[(&str, Civil)], minutes: i64) -> (Game, Outcome) {
    let mut g = game(case);
    let p: Vec<usize> = prio.iter().map(|n| idx(&g, n)).collect();
    let mut plan = Plan::new(g.districts.len()).with_priorities(&p);
    for (n, c) in civil {
        plan = plan.with_civil(idx(&g, n), *c);
    }
    g.commit(plan).unwrap();
    g.run_until(minutes * 60).unwrap();
    let o = g.outcome();
    (g, o)
}

#[test]
fn same_seed_same_plan_same_game() {
    let (a, oa) = run("Piano1", &["Grilli", "Ghiande"], &[("Grilli", Civil::Preallerta)], 60);
    let (b, ob) = run("Piano1", &["Grilli", "Ghiande"], &[("Grilli", Civil::Preallerta)], 60);
    assert_eq!(oa, ob);
    assert_eq!(a.log, b.log);
    let pos = |g: &Game| g.crews.units.iter().map(|u| (u.pos.x, u.pos.y)).collect::<Vec<_>>();
    assert_eq!(pos(&a), pos(&b));
}

#[test]
fn inverting_priorities_changes_posts_and_outcomes() {
    let start = |prio: &[&str]| {
        let mut g = game("Piano3");
        let p: Vec<usize> = prio.iter().map(|n| idx(&g, n)).collect();
        g.commit(Plan::new(g.districts.len()).with_priorities(&p)).unwrap();
        g
    };
    let (mut ga, mut gb) = (start(&["Grilli", "Ghiande"]), start(&["Ghiande", "Grilli"]));
    let (piano, coste) = (idx(&ga, "Grilli"), idx(&ga, "Ghiande"));
    let on = |g: &Game, d: usize| g.posts.iter().flatten().filter(|p| p.district == d).count();
    // the first-ranked district gets two units once the plan is under way (at
    // the end one may be between a refill and its post)
    ga.run_until(30 * 60).unwrap();
    gb.run_until(30 * 60).unwrap();
    assert_eq!(on(&ga, piano), 2, "{:?}", ga.posts);
    assert_eq!(on(&gb, coste), 2, "{:?}", gb.posts);
    ga.run_until(180 * 60).unwrap();
    gb.run_until(180 * 60).unwrap();
    let (a, b) = (ga.outcome(), gb.outcome());
    // and it is the homes that change, not only the icons
    assert!(
        a.districts[piano].homes_hit < b.districts[piano].homes_hit,
        "Pian dei Grilli first should lose fewer homes: {} vs {}",
        a.districts[piano].homes_hit,
        b.districts[piano].homes_hit
    );
}

#[test]
fn defence_reduces_simulated_exposure() {
    let (g, defended) = run("Piano1", &["Grilli"], &[], 180);
    let (_, open) = run("Piano1", &[], &[], 180);
    let d = idx(&g, "Grilli");
    assert!(defended.districts[d].homes_hit < open.districts[d].homes_hit, "{} vs {}", defended.districts[d].homes_hit, open.districts[d].homes_hit);
}

fn departed(g: &Game, d: usize) -> usize {
    g.districts[d].households.iter().filter(|&&i| matches!(g.agents.households[i].status, Status::Evacuating | Status::Evacuated)).count()
}

#[test]
fn prealert_is_not_an_evacuation() {
    // Castelvento with the fire 800 m north: at T+30 an evacuation has most of
    // the town on the move, a pre-alert has it ready at home.
    let (gp, _) = run("Borgo1", &[], &[("Castelvento", Civil::Preallerta)], 30);
    let (ge, _) = run("Borgo1", &[], &[("Castelvento", Civil::Evacua)], 30);
    let (gn, _) = run("Borgo1", &[], &[], 30);
    let d = idx(&gp, "Castelvento");
    let n = gp.districts[d].households.len();
    let ready = gp.districts[d].households.iter().filter(|&&i| gp.agents.households[i].readied_s > 0.0).count();
    assert!(departed(&ge, d) > n / 3, "evacuation: {} of {n} gone", departed(&ge, d));
    assert!(departed(&gp, d) <= departed(&gn, d) + n / 20, "pre-alert must not send people away: {} vs {}", departed(&gp, d), departed(&gn, d));
    assert!(ready > 9 * n / 10, "pre-alerted households get ready: {ready} of {n}");
}

#[test]
fn prealert_pays_off_when_the_fire_comes() {
    // Pian dei Grilli with the fire coming: pre-alerted households leave faster when
    // they do decide to, so fewer are caught at home than with no order.
    let (g, pre) = run("Piano3", &[], &[("Grilli", Civil::Preallerta)], 120);
    let (_, none) = run("Piano3", &[], &[], 120);
    let d = idx(&g, "Grilli");
    assert!(pre.districts[d].caught < none.districts[d].caught, "{} vs {}", pre.districts[d].caught, none.districts[d].caught);
}

#[test]
fn preview_has_no_side_effects_and_commit_revalidates() {
    let mut a = game("Piano1");
    let mut b = game("Piano1");
    let plan = |g: &Game| Plan::new(g.districts.len()).with_priorities(&[idx(g, "Grilli"), idx(g, "Ghiande")]);
    a.commit(plan(&a)).unwrap();
    b.commit(plan(&b)).unwrap();
    a.run_until(20 * 60).unwrap();
    b.run_until(20 * 60).unwrap();
    // A previews the inverted plan and an evacuation, then carries on
    let inverted = Plan::new(a.districts.len()).with_priorities(&[idx(&a, "Ghiande"), idx(&a, "Grilli")]).with_civil(idx(&a, "Grilli"), Civil::Evacua);
    let preview = a.preview(&inverted).unwrap();
    assert_ne!(preview.posts, a.posts, "the preview proposes something else");
    assert!(a.agents.households.iter().all(|h| !h.ordered), "a preview orders nobody");
    a.run_until(40 * 60).unwrap();
    b.run_until(40 * 60).unwrap();
    assert_eq!(a.outcome(), b.outcome());
    assert_eq!(a.log, b.log);
    // committing re-plans from the state now, not from the old preview
    let committed = a.commit(inverted.clone()).unwrap();
    assert_eq!(committed, a.preview(&inverted).unwrap());
}

#[test]
fn civil_orders_only_escalate() {
    let mut g = game("Borgo1");
    let d = idx(&g, "Castelvento");
    g.commit(Plan::new(g.districts.len()).with_civil(d, Civil::Evacua)).unwrap();
    g.commit(Plan::new(g.districts.len()).with_civil(d, Civil::Preallerta)).unwrap();
    assert_eq!(g.active.civil[d], Civil::Evacua);
}

/// The card's defence line comes from the units' real state: a unit sent to
/// the first priority is first on its way, with minutes left that shrink,
/// then on its post.
#[test]
fn arrivals_follow_the_units() {
    let mut g = game("Coste2_gira");
    let near = g.district_index(&g.case.near.clone()).unwrap();
    g.commit(Plan::new(g.districts.len()).with_priorities(&[near])).unwrap();
    assert!(g.arrivals(near).iter().any(|(_, a)| matches!(a, rocca::Arrival::InMin(_))), "a unit should be on its way: {:?}", g.arrivals(near));
    let mut arrived = false;
    let mut last = vec![u32::MAX; g.crews.units.len()];
    while g.time_s() < 30 * 60 && !arrived {
        g.run_until(g.time_s() + 60).unwrap();
        for (k, a) in g.arrivals(near) {
            match a {
                rocca::Arrival::OnPost => arrived = true,
                rocca::Arrival::InMin(m) => {
                    assert!(m <= last[k].saturating_add(1), "unit {k}: minutes left grew, {m} after {}", last[k]);
                    last[k] = m;
                }
                _ => {}
            }
        }
    }
    assert!(arrived, "no unit reached its post in 30 min");
}

/// «Come sono andati i tuoi ordini»: evacuating a place the fire stays far
/// from is called out, a place it came near but never reached is said as a
/// fact (Misura counts both), and families the fire reached at home are a
/// missed or late order.
#[test]
fn the_review_judges_each_order_by_the_fire() {
    let all = ["Castelvento", "Pian dei Grilli", "Fornaci", "San Rocco", "Le Terrazze", "Le Ghiande"];
    let evac: Vec<(&str, Civil)> = all.iter().map(|&n| (n, Civil::Evacua)).collect();
    let (g, _) = run("Rocco3", &["San Rocco"], &evac, 180);
    let far = g.order_review(idx(&g, "Pian dei Grilli"));
    assert_eq!(far.verdict, Verdict::Forse, "{far:?}");
    assert!(far.text.contains("Forse bastava la preallerta"), "{far:?}");
    assert!(g.needless_evacuations() > 0);
    let near = g.order_review(idx(&g, "Le Terrazze"));
    assert!(near.verdict == Verdict::Forse && near.text.contains("non alle case"), "{near:?}");
    // no orders: nothing evacuated in vain, families caught where it struck
    let (g, _) = run("Rocco3", &["San Rocco"], &[], 180);
    assert_eq!(g.needless_evacuations(), 0);
    let hit = g.order_review(idx(&g, "San Rocco"));
    assert!(hit.verdict == Verdict::Male && hit.text.contains("colte in casa"), "{hit:?}");
}

/// The debrief's lines come from what happened: an evacuation order shows up
/// with its time and the families that left, and the place no unit was sent
/// to says so.
#[test]
fn the_story_tells_what_happened() {
    let (g, _) = run("Borgo2", &["Castelvento"], &[("Castelvento", Civil::Evacua)], 180);
    let town = g.story(idx(&g, "Castelvento"));
    assert!(town[0].starts_with("Evacuazione alle 14:00: dopo l'ordine sono partite "), "{town:?}");
    // the end state adds up to the district
    assert!(town[1].starts_with("Alla fine, su 190 famiglie: "), "{town:?}");
    assert!(town[2].starts_with("Mezzi in postazione per"), "{town:?}");
    let other = g.story(idx(&g, "Grilli"));
    assert!(other.iter().any(|l| l == "Nessun mezzo: non era tra le priorità."), "{other:?}");
}


/// The mayor's rule (`game.json`, 2026-10-13): no evacuation before her
/// authorisation; she gives it as a crisis about one district, and after it
/// that district's evacuation is accepted.
#[test]
fn evacuation_waits_for_the_mayor() {
    let mut g = Game::new(&data(), "Rocco3", 1).unwrap();
    assert!(g.mayor.is_some(), "game.json turns the mayor's rule on");
    let n = g.districts.len();
    let near = idx(&g, &g.case.near.clone());
    let early = Plan::new(n).with_priorities(&[near]).with_civil(near, Civil::Evacua);
    assert!(g.commit(early).is_err(), "evacuation before the authorisation");
    g.commit(Plan::new(n).with_priorities(&[near]).with_civil(near, Civil::Preallerta)).unwrap();
    let mut asked = None;
    while asked.is_none() && g.time_s() < g.case.duration_s() {
        g.step().unwrap();
        if let Some(c) = g.take_crisis() {
            assert!(matches!(c.kind, rocca::crisis::Kind::Autorizzazione { .. }), "with the mayor, her crises only: {c:?}");
            asked = c.kind.district();
        }
    }
    let d = asked.expect("Rocco3 threatens a district: the mayor authorises it");
    assert!(g.may_evacuate(d) && g.authorised_at(d).is_some());
    assert!(g.log.iter().any(|e| e.text.starts_with("la sindaca autorizza")));
    let plan = g.active.clone().with_civil(d, Civil::Evacua);
    g.commit(plan).unwrap();
    assert_eq!(g.active.civil[d], Civil::Evacua);
}
