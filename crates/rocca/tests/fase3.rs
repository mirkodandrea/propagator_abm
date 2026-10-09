//! Phase 3 checks (03-PIANO-DI-AZIONE, fase 3; CLAUDE.md «Verifiche») on the
//! game's territory, `data/scenarios/rocca_ventosa`.

use std::path::{Path, PathBuf};

use rocca::{Civil, Game, Outcome, Plan};
use scenario::population::Status;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn game(case: &str) -> Game {
    Game::new(&data(), case, 1).unwrap()
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
    let (ga, a) = run("Piano1", &["Grilli", "Ghiande"], &[], 180);
    let (gb, b) = run("Piano1", &["Ghiande", "Grilli"], &[], 180);
    let (piano, coste) = (idx(&ga, "Grilli"), idx(&ga, "Ghiande"));
    let on = |g: &Game, d: usize| g.posts.iter().flatten().filter(|p| p.district == d).count();
    // the first-ranked district gets two units
    assert_eq!(on(&ga, piano), 2, "{:?}", ga.posts);
    assert_eq!(on(&gb, coste), 2, "{:?}", gb.posts);
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
    let (g, pre) = run("Piano1", &[], &[("Grilli", Civil::Preallerta)], 120);
    let (_, none) = run("Piano1", &[], &[], 120);
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

/// The debrief's lines come from what happened: an evacuation order shows up
/// with its time and the families that left, and the place no unit was sent
/// to says so.
#[test]
fn the_story_tells_what_happened() {
    let (g, _) = run("Borgo2", &["Castelvento"], &[("Castelvento", Civil::Evacua)], 180);
    let town = g.story(idx(&g, "Castelvento"));
    assert!(town[0].starts_with("Evacuazione alle 14:00: dopo l'ordine sono partite "), "{town:?}");
    // the end state adds up to the district
    assert!(town[1].starts_with("Alla fine, su 160 famiglie: "), "{town:?}");
    assert!(town[2].starts_with("Mezzi in postazione per"), "{town:?}");
    let other = g.story(idx(&g, "Grilli"));
    assert!(other.iter().any(|l| l == "Nessun mezzo: non era tra le priorità."), "{other:?}");
}
