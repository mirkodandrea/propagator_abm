//! Spec 5.1 / 5.3: money is a pure function of the action log, and with it on the
//! forecast is a real decision -- "always evacuate" and "never" both lose to a
//! policy that reads the forecast, and every fixed policy wins on some seeds.

use demo::cost::{Action, Log, Weights, EVACUATION_PER_HOUSEHOLD_EUR};
use demo::policy::Policy;
use demo::sweep::{regret_by, run_grid, Record};
use demo::weather::draw;
use demo::{Run, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

/// The twin and the live game price identically because both feed one `Log` and
/// pricing reads nothing else: rebuilding the log entry by entry gives the same bill.
#[test]
fn cost_is_a_pure_function_of_the_action_log() {
    for id in ALL {
        let d = draw(id, 4).unwrap();
        let mut run = Run::new(&data_dir(), d.spec, 4).unwrap();
        Policy::evacuate_and_units(5, 3).play(&mut run, &d).unwrap();
        let mut rebuilt = Log::default();
        for e in &run.log.entries {
            rebuilt.push(*e);
        }
        assert_eq!(rebuilt.price(run.time_s()), run.ledger(), "{id}");
        assert!(run.ledger().total_eur() > 0.0, "{id}: a session with orders cost nothing");
        // Time moves the bill (engines are on call-out by the hour), never backwards.
        assert!(run.log.price(run.time_s()).total_eur() >= run.log.price(run.time_s() / 2).total_eur());
    }
}

#[test]
fn doing_nothing_costs_nothing_and_an_evacuation_costs_per_household() {
    for id in ALL {
        let d = draw(id, 6).unwrap();
        let mut idle = Run::new(&data_dir(), d.spec, 6).unwrap();
        Policy::none().play(&mut idle, &d).unwrap();
        assert_eq!(idle.ledger().total_eur(), 0.0, "{id}");

        let mut evac = Run::new(&data_dir(), d.spec, 6).unwrap();
        Policy::evacuate(0).play(&mut evac, &d).unwrap();
        let n = evac.agents.households.len() as f32;
        assert_eq!(evac.ledger().by_action(Action::Evacuation), n * EVACUATION_PER_HOUSEHOLD_EUR, "{id}");
        // A needless evacuation bills like a needed one: nothing in the log says why.
        assert_eq!(evac.ledger().total_eur(), n * EVACUATION_PER_HOUSEHOLD_EUR, "{id}");
    }
}

/// The gate for the forecast being a decision (5.1/5.3). At the analysis weights
/// (EUR 20 k per family caught at home and per home reached -- see `cost::Weights`;
/// `tests/cost_sweep.rs` reports it across 5-80 k) a forecast-follower has lower
/// mean loss than both "evacuate at T+0 always" and "never evacuate", on every
/// town; and every fixed policy is the best one on *some* seeds.
#[test]
fn the_forecast_beats_always_and_never_and_nobody_always_wins() {
    let policies = Policy::fixed_set();
    let (none, t0, follow) = (0, 1, 5);
    assert_eq!(policies[none].name, "none");
    assert_eq!(policies[t0].name, "evac T+0");
    assert_eq!(policies[follow].name, "follow-forecast");
    let w = Weights::ANALYSIS;
    let rs = run_grid(&data_dir(), &ALL, &policies, &[Default::default()], 1..=24);
    let loss = |r: &Record| w.loss(r.eur, r.out.caught, r.out.homes_lost) / 1000.0;
    for id in ALL {
        let mean = |p: usize| rs.iter().filter(|r| r.town == id && r.policy == p).map(&loss).sum::<f32>() / 24.0;
        println!("{id}: never {:.0}k  always {:.0}k  follow {:.0}k", mean(none), mean(t0), mean(follow));
        assert!(mean(follow) < mean(t0), "{id}: evacuating at T+0 always beats reading the forecast");
        assert!(mean(follow) < mean(none), "{id}: doing nothing beats reading the forecast");
        for p in [none, t0] {
            let (_, wins) = regret_by(&rs, id, p, 0, loss);
            assert!(wins > 0.0, "{id}: {} is never the best policy on any seed", policies[p].name);
        }
        let (regret_t0, _) = regret_by(&rs, id, t0, 0, loss);
        let (regret_none, _) = regret_by(&rs, id, none, 0, loss);
        assert!(regret_t0 > 5.0 && regret_none > 5.0, "{id}: a fixed policy has trivial regret ({regret_t0}, {regret_none})");
    }
}
