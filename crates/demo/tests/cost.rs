//! Spec 5.1 / 5.3: money is a pure function of the action log, and with it on the
//! forecast is a real decision -- "always evacuate" and "never" both lose to a
//! policy that reads the forecast, and every fixed policy wins on some seeds.

use demo::cost::{Action, Log, EVACUATION_PER_HOUSEHOLD_EUR};
use demo::policy::Policy;
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
        for e in &run.referee.log.entries {
            rebuilt.push(*e);
        }
        assert_eq!(rebuilt.price(run.time_s()), run.ledger(), "{id}");
        assert!(run.ledger().total_eur() > 0.0, "{id}: a session with orders cost nothing");
        // Time moves the bill (engines are on call-out by the hour), never backwards.
        assert!(run.referee.log.price(run.time_s()).total_eur() >= run.referee.log.price(run.time_s() / 2).total_eur());
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

// The forecast gate (`follow-forecast` beats always/never) was a property of the
// one-order game. The district game's equivalent is `tests/districts.rs`.
