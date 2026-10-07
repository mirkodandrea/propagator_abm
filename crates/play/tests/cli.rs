//! The `play` binary end to end (playtest spec §1): a whole session by
//! commands, in a scratch directory, the way the playtester runs it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("play-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn play(dir: &Path, args: &[&str]) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_play")).args(args).current_dir(dir).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap() + &String::from_utf8(out.stderr).unwrap();
    (text, out.status.success())
}

/// Nothing English, nothing that looks like a Rust error.
fn italian(s: &str) {
    for w in ["panicked", "Error", "error:", "thread '", "RUST_BACKTRACE", "Some(", "None", "unwrap"] {
        assert!(!s.contains(w), "not for a player: {w:?} in\n{s}");
    }
}

#[test]
fn a_whole_session_by_commands() {
    let dir = scratch("session");
    let (o, ok) = play(&dir, &["mostra"]);
    assert!(!ok && o.contains("./play nuova"), "{o}");
    italian(&o);

    let (o, ok) = play(&dir, &["nuova", "--seme", "3"]);
    assert!(ok, "{o}");
    assert!(o.contains("Turno 1 di 5") && o.contains("RISORSE") && o.contains("QUARTIERI"), "{o}");
    assert!(o.contains("ordini proseguono da soli") && o.contains("fino a T+60") && o.contains("annulla"), "{o}");
    assert!(o.lines().all(|l| l.chars().count() <= 100), "a line wider than 100 columns");
    italian(&o);

    let (o, ok) = play(&dir, &["scegli", "K"]);
    assert!(ok && o.lines().last() == Some("> ./play ordina K · ./play scegli <altra risorsa> · ./play avanti"), "{o}");

    for bad in [&["scegli", "X"][..], &["ordina", "E1", "abc"], &["ordina", "E3", "1"], &["ordina", "P", "4"], &["vola"]] {
        let (o, ok) = play(&dir, bad);
        assert!(!ok && o.lines().count() == 1, "{bad:?}: {o}");
        italian(&o);
    }

    let (o, ok) = play(&dir, &["scegli", "E1"]);
    assert!(ok && o.contains("HAI SCELTO") && o.contains("arriva in"), "{o}");
    assert!(!o.contains("   N +") && o.contains("./play mostra"), "compact decision output: {o}");
    assert!(o.contains("aggiuntive") && o.contains("fuori dalla direzione prevista"), "causal previews: {o}");
    for cmd in [&["ordina", "P", "1"][..], &["ordina", "E1", "1"], &["ordina", "E2", "1"], &["ordina", "K"]] {
        let (o, ok) = play(&dir, cmd);
        assert!(ok, "{cmd:?}: {o}");
    }
    let (o, ok) = play(&dir, &["annulla", "E2"]);
    assert!(ok && !o.contains("Autobotte 2   libera · acqua 100 %\n        → ORDINE"), "{o}");
    for turn in 1..=5 {
        let (o, ok) = play(&dir, &["avanti"]);
        assert!(ok, "turn {turn}: {o}");
        italian(&o);
        assert!(!o.contains("   N +"), "turn output should omit repeated maps: {o}");
        assert!(o.lines().all(|l| l.chars().count() <= 100), "turn output wider than 100 columns: {o}");
        if turn == 2 { assert!(o.contains("RINFORZO ARRIVATO"), "{o}"); }
        assert!(o.contains("RAPPORTO") || turn == 5, "turn {turn}: no report\n{o}");
        if turn == 5 {
            assert!(o.contains("FINE DELL'INCENDIO") && o.contains("senza ordini") && o.contains("Famiglie bloccate dal fuoco"), "{o}");
            assert!(!o.contains("Famiglie in salvo"), "the verdict must show the caught-family comparison: {o}");
            assert_eq!(o.matches("Famiglie bloccate dal fuoco:").count(), 4, "overall and per-district comparisons: {o}");
        }
    }
    let (o, ok) = play(&dir, &["avanti"]);
    assert!(!ok && o.contains("riprova"), "{o}");

    // The whole session is replayed from partita.json on every command: the
    // spec's budget is 2 s in release.
    let t = Instant::now();
    let (o, ok) = play(&dir, &["mostra"]);
    let took = t.elapsed();
    assert!(ok && o.contains("FINE DELL'INCENDIO"), "{o}");
    if !cfg!(debug_assertions) {
        assert!(took.as_secs_f32() < 2.0, "replay of a full session took {took:?}");
    }
    println!("replay of a full session with its counterfactual: {took:?}");

    // Same seed, same orders, same verdict (riprova, then the same orders).
    let first = o;
    let (_, ok) = play(&dir, &["riprova"]);
    assert!(ok);
    for cmd in [&["ordina", "P", "1"][..], &["ordina", "E1", "1"], &["ordina", "K"]] {
        assert!(play(&dir, cmd).1);
    }
    for _ in 0..5 {
        assert!(play(&dir, &["avanti"]).1);
    }
    let (again, _) = play(&dir, &["mostra"]);
    assert_eq!(first, again);

    // The seed is kept for the harness, never printed.
    let saved = std::fs::read_to_string(dir.join("partita.json")).unwrap();
    assert!(saved.contains("\"semi\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn help_is_five_lines_and_stato_is_not_in_it() {
    let dir = scratch("help");
    let (o, ok) = play(&dir, &["aiuto"]);
    assert!(ok);
    assert_eq!(o.trim_end().lines().count(), 5, "{o}");
    assert!(!o.contains("stato"));
    italian(&o);
    let _ = std::fs::remove_dir_all(&dir);
}
