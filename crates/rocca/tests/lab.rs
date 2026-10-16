//! The suppression laboratory runs on a synthetic territory and the teams, led
//! ahead of the front, spare hectares against the same fire with no orders.

use rocca::lab::run::{baseline, run, Job};
use rocca::lab::synth;
use rocca::game::Tuning;

#[test]
fn teams_ahead_of_the_front_spare_hectares_on_a_synthetic_plain() {
    std::env::set_var("ROCCA_SCENARIO", synth::SCENARIO);
    let out = std::env::temp_dir().join("rocca-lab-test");
    let spec = synth::spec("pianura").unwrap();
    let built = synth::build(&spec, &out, std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"))).unwrap();
    let job = Job { scenario: spec.name.clone(), data_dir: built.data_dir, strategy: "lead".into(), teams: 2, seed: 1, tuning: Tuning::default(), tune_tag: String::new(), aim: Default::default(), without: None };
    let base = baseline(&job).unwrap();
    let r = run(&job, &base, false).unwrap();
    assert!(base.ha > 20.0, "the fire with no orders must burn: {} ha", base.ha);
    assert!(r.metrics.get("saved_ha").unwrap() > 0.0, "{:?}", r.metrics.get("ha"));
    assert!(r.metrics.get("litres").unwrap() > 0.0);
}
