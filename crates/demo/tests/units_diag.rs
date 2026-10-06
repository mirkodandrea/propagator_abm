//! What do the units actually do in a drawn session?
//! `cargo test -p demo --release --test units_diag -- --ignored --nocapture`
use abm::suppression::UnitEffect;
use demo::policy::Policy;
use demo::weather::draw;
use demo::{Run, Variant, ALL};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").canonicalize().unwrap()
}

/// Why does protection bite in valle and not in borgo and porto?
#[test]
#[ignore = "report"]
fn defence_diag() {
    for id in ["demo_borgo", "demo_valle", "demo_porto"] {
        let d = draw(id, 3).unwrap();
        let mut run = Run::with_variant(&data_dir(), d.spec, 3, Variant { defend_homes: true, ..Variant::default() }).unwrap();
        let p = Policy::protect(10);
        let mut last = -1;
        let o = p
            .play_observed(&mut run, &d, |r| {
                let m = r.time_s() / 300;
                if m != last {
                    last = m;
                    let eng: Vec<String> = r
                        .crews
                        .units
                        .iter()
                        .filter(|u| u.kind == abm::suppression::UnitKind::Engine)
                        .map(|u| format!("{:?}/{:.0}L", u.state, u.water_l))
                        .collect();
                    println!(
                        "  {id} T+{:>2}: head->town {:>5.0} m  defended {:>3}  engines {}",
                        r.time_s() / 60,
                        r.head_to_town_m(),
                        r.tally.defended_now(r.time_s()),
                        eng.join(" ")
                    );
                }
            })
            .unwrap();
        println!("{id}: {o:?}");
        for u in &run.crews.units {
            println!("   {:<10} {:?} water {:.0} note '{}'", u.kind.label(), u.state, u.water_l, u.note);
        }
    }
}

#[test]
#[ignore = "report"]
fn units_diag() {
    for id in ALL {
        for x in [1.0f32, 8.0] {
            let d = draw(id, 3).unwrap();
            let mut run =
                Run::with_variant(&data_dir(), d.spec, 3, Variant { unit_effect: UnitEffect::all(x), ..Variant::default() }).unwrap();
            let p = Policy::units(3);
            p.play(&mut run, &d).unwrap();
            let s = run.crews.stats();
            println!("{id} x{x}: water {:.0} L  line {:.0} m  drops {}  ha {:.1}", s.water_l, s.line_m, s.drops, run.outcome().hectares);
            for u in &run.crews.units {
                println!(
                    "   {:<12} {:?} {:?} water_used {:.0} line {:.0} drops {} note '{}'",
                    u.kind.label(), u.state, u.task, u.water_used_l, u.line_cut_m, u.drops, u.note
                );
            }
        }
    }
}
