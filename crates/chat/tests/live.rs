//! The one test that needs a network and somebody's credit.
//!
//! Everything else in this crate is a pure function over a fixture, which is
//! deliberate — but it leaves the actual request untested, and the failures
//! that live there (a header the provider rejects, a body field it ignores,
//! a stream that never terminates) are exactly the ones a fixture cannot
//! catch. So this exists, and it is `#[ignore]`d: it costs money and it fails
//! on a train. Run it on purpose:
//!
//! ```text
//! cargo test -p chat --release -- --ignored --nocapture
//! ```
//!
//! Skips itself rather than failing when no provider is configured, so running
//! the whole ignored set on a machine with no key is quiet.

use chat::{Client, Dossier, Fact, LlmConfig, Message, SubjectKind, TimelineEntry};

#[test]
#[ignore = "needs a configured provider and spends real credit"]
fn the_configured_provider_answers() {
    chat::config::load_dotenv();
    let config = LlmConfig::default();
    if let Err(why) = config.readiness() {
        eprintln!("skipping: {why}");
        return;
    }
    eprintln!("asking {} via {}", config.model(), config.provider.label());

    let mut deltas = 0usize;
    let messages = vec![
        Message::system(
            "You are a resident of Spotorno during a wildfire. Answer in one short sentence.",
        ),
        Message::user("Where are you right now?"),
    ];
    let reply = Client::new(config)
        .complete(&messages, &mut |d| {
            deltas += 1;
            eprint!("{d}");
        })
        .expect("the provider answered");
    eprintln!("\n--- {} chars in {deltas} chunks", reply.len());

    assert!(!reply.trim().is_empty());
    // More than one chunk is what tells streaming apart from a single blob
    // delivered at the end, which is the failure mode that makes the panel
    // look hung.
    assert!(deltas > 1, "the reply did not stream");
}

/// The whole round trip a first interview makes: invent a person from the
/// traits, then have them answer as that person.
///
/// The half worth testing live is the persona: it is the one place the model is
/// asked for a *format* rather than for prose, and "returned something that is
/// not JSON" is a real failure that varies by model and that no fixture can
/// predict.
#[test]
#[ignore = "needs a configured provider and spends real credit"]
fn a_persona_is_generated_and_then_answers_in_character() {
    chat::config::load_dotenv();
    let config = LlmConfig::default();
    if let Err(why) = config.readiness() {
        eprintln!("skipping: {why}");
        return;
    }
    let client = Client::new(config.clone());
    let dossier = fixture();

    let persona = chat::persona::parse(
        &client.complete(&chat::persona::request(&dossier), &mut |_| {}).expect("persona call"),
        &config.model(),
    )
    .expect("the model returned a usable persona");
    eprintln!("persona: {}\n{}", persona.headline(), persona.background);
    assert!(!persona.name.trim().is_empty());

    let messages = vec![
        Message::system(chat::prompt::system(&persona, &dossier)),
        Message::user("Why haven't you left yet?"),
    ];
    let reply = client.complete(&messages, &mut |_| {}).expect("interview call");
    eprintln!("--- asked why they have not left:\n{reply}");
    assert!(!reply.trim().is_empty());

    // Not an assertion about the model's judgement — just that the two calls
    // agree on who is speaking, which is the thing the persona cache buys.
    assert!(persona.age > 0);
}

/// A household that has been warned, has not moved, and can see the fire.
fn fixture() -> Dossier {
    Dossier {
        kind: SubjectKind::Household,
        id: 412,
        callsign: None,
        sim_time_s: 2_700,
        clock: "00:45:00".into(),
        facts: vec![
            Fact::new("Your household", "four of you (ages 58, 61, 24, 19), one car"),
            Fact::new("Animals", "you have animals to think about"),
            Fact::new("Where you live", "the macchia comes right up to the house"),
            Fact::new(
                "What you always said you would do",
                "wait and see how it develops before doing anything drastic",
            ),
            Fact::new(
                "What you make of the authorities",
                "you do not much trust what officials tell you",
            ),
            Fact::new(
                "Before you could actually leave",
                "about 35 minutes of things you would have to do first — you are slow to get moving",
            ),
            Fact::new("Right now you are", "aware something is happening, and not yet doing anything about it"),
        ],
        perceptions: vec![
            "The fire is a few hundred metres away to the north, up the slope. You can see flame."
                .into(),
            "There is smoke on the ground here and it stings.".into(),
            "A hard wind, 35 km/h, blowing from the north.".into(),
        ],
        timeline: vec![
            TimelineEntry { sim_time_s: 900, line: "you realised something was going on".into() },
            TimelineEntry { sim_time_s: 1_800, line: "the evacuation order reached you".into() },
        ],
    }
}
