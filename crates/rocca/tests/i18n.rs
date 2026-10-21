//! Every language of the picker has a catalog that covers the whole game, and
//! the game speaks it (`tools/i18n/keys.py --check` checks the catalogs against
//! the sources; this checks that the engine can read them and uses them).

use std::path::Path;

use rocca::i18n;

fn data() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

// One test: the language is a global of the process, tests run in parallel threads.
#[test]
fn every_language_loads_translates_and_changes_what_the_game_says() {
    i18n::set_root(data());
    let keys: Vec<String> = serde_json::from_slice(&std::fs::read(data().join("i18n/keys.json")).unwrap()).unwrap();
    for (code, _) in i18n::LANGS.iter().chain([&i18n::EASTER_EGG]) {
        assert_eq!(i18n::set(code), *code, "{code}: the catalog does not load");
        if *code == "it" {
            continue;
        }
        let cat: std::collections::HashMap<String, String> = serde_json::from_slice(&std::fs::read(data().join(format!("i18n/{code}.json"))).unwrap()).unwrap();
        for k in &keys {
            assert!(cat.get(k).is_some_and(|t| !t.trim().is_empty()), "{code}: no translation for {k:?}");
        }
    }
    i18n::set("it");
    assert_eq!(i18n::code(), "it");
    assert_eq!(rocca::words::compass(94.0), "est");
    i18n::set("en");
    assert_eq!(rocca::words::compass(94.0), "east");
    assert_eq!(rocca::words::compass(200.0), "south");
    i18n::set("it");
}
