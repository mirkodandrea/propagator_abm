//! What the agent is allowed to know, and how it is said to them.
//!
//! A [`Dossier`] is one agent's whole world: the traits the population bake
//! gave them, what they can perceive from where they are standing right now,
//! and the timeline of discrete things that have happened to them, straight
//! out of the run's event log. It is assembled by `game` — the only crate with
//! a `Sim` to assemble it from — and it is the *complete* input to the
//! interview. Nothing else reaches the model.
//!
//! **There is no field here for an incident-wide fact, and that is the
//! feature.** The tempting version of this type has a `status: &Stats` on it,
//! because the caller has one and it would make the answers so much better
//! informed. It would also make every answer wrong in a way that is invisible:
//! a resident who says "about two hundred of us got out" is reporting the
//! simulation's own telemetry in the first person, and there is no way to tell
//! that from a well-simulated resident except by knowing what a resident could
//! possibly know. The type is the enforcement — a caller cannot leak what the
//! struct cannot carry — and [`system`] says the same thing again in words, so
//! that a model handed something borderline in a fact still refuses it.
//!
//! **Numbers go in; numbers do not come out.** The dossier carries
//! `risk_perception = 0.31` because that is what actually drives the agent,
//! and the prompt's job is to have the person *speak* it rather than quote it.
//! An agent that says "my risk perception is 0.31" has told you nothing you
//! could not read in the inspector, which is the panel directly above this
//! one.

use crate::persona::Persona;
use crate::subject::SubjectKind;

/// One named quantity from the model, already turned into words the agent
/// could plausibly own. `("Preparation", "about 20 minutes of things to do
/// before we could leave")` rather than `("prep_time_min", "18.4")`.
#[derive(Debug, Clone)]
pub struct Fact {
    pub label: String,
    pub value: String,
}

impl Fact {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Fact {
        Fact { label: label.into(), value: value.into() }
    }
}

/// One thing that happened to this agent, at the simulated time it happened.
#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub sim_time_s: i64,
    pub line: String,
}

/// Everything one agent knows, and nothing else.
#[derive(Debug, Clone)]
pub struct Dossier {
    pub kind: SubjectKind,
    /// The agent's index in its own list — carried for the transcript's sake,
    /// never shown to the model. A resident does not know they are household
    /// 412.
    pub id: i64,
    /// A suppression unit's callsign (`Autobotte 2`). `None` for civilians,
    /// who do not have one.
    pub callsign: Option<String>,
    /// Simulated seconds since ignition, and the same as `HH:MM:SS`.
    pub sim_time_s: i64,
    pub clock: String,
    /// Baked traits and current state, in the agent's own terms.
    pub facts: Vec<Fact>,
    /// What they can see, hear and feel from where they are, right now.
    pub perceptions: Vec<String>,
    /// Their own row of the run's event log, oldest first.
    pub timeline: Vec<TimelineEntry>,
}

impl Dossier {
    /// `T+01:23` for a timeline line. Seconds are dropped deliberately: a
    /// person recalling a fire says "about twenty past", and a model handed
    /// `T+01:23:04` will happily repeat the seconds back.
    pub fn stamp(sim_time_s: i64) -> String {
        format!("T+{:02}:{:02}", sim_time_s / 3600, (sim_time_s / 60) % 60)
    }
}

/// The system prompt for an interview: who they are, what they know, and the
/// rules about what they may not.
pub fn system(persona: &Persona, dossier: &Dossier) -> String {
    let mut s = String::new();

    let who = match dossier.kind {
        SubjectKind::Household => "a resident of a small town on the Ligurian coast of Italy \
             — Spotorno, Bergeggi or Noli — speaking for your household",
        SubjectKind::Person => "a resident of a small town on the Ligurian coast of Italy \
             — Spotorno, Bergeggi or Noli — who is away from home and not with your family",
        SubjectKind::Unit => "an Italian firefighter working a wildfire on the Ligurian coast, \
             speaking for your crew",
    };

    s.push_str(&format!("You are {who}. A wildfire is burning.\n\n"));

    if persona.is_anonymous() {
        s.push_str(
            "You have no name on record. Speak as yourself anyway, and do not invent \
             biographical detail beyond what the facts below give you.\n\n",
        );
    } else {
        s.push_str(&format!(
            "You are {name}, {age}. {occupation}. {role}.\nYou speak like this: {voice}.\n\
             {background}\n\n",
            name = persona.name,
            age = persona.age,
            occupation = persona.occupation,
            role = persona.role,
            voice = persona.voice,
            background = persona.background,
        ));
    }
    if let Some(cs) = &dossier.callsign {
        s.push_str(&format!("Your callsign is {cs}.\n\n"));
    }

    s.push_str(&format!(
        "It is now {clock} since the fire started — about {mins} minutes.\n\n",
        clock = dossier.clock,
        mins = dossier.sim_time_s / 60,
    ));

    s.push_str("WHAT IS TRUE OF YOU\n");
    for f in &dossier.facts {
        s.push_str(&format!("- {}: {}\n", f.label, f.value));
    }
    s.push('\n');

    if !dossier.perceptions.is_empty() {
        s.push_str("WHAT YOU CAN SEE AND FEEL RIGHT NOW\n");
        for p in &dossier.perceptions {
            s.push_str(&format!("- {p}\n"));
        }
        s.push('\n');
    }

    s.push_str("WHAT HAS HAPPENED TO YOU\n");
    if dossier.timeline.is_empty() {
        s.push_str("- Nothing yet. The fire is somewhere out there and your day is normal.\n");
    } else {
        for e in &dossier.timeline {
            s.push_str(&format!("- {} {}\n", Dossier::stamp(e.sim_time_s), e.line));
        }
    }
    s.push('\n');

    let speaker = match dossier.kind {
        SubjectKind::Unit => {
            "The person speaking to you is the incident commander, over the radio."
        }
        _ => {
            "The person speaking to you may be the incident commander, a firefighter, or a \
             researcher asking you afterwards what you did. Answer whoever it sounds like."
        }
    };

    s.push_str(&format!(
        "HOW TO ANSWER\n\
         {speaker}\n\
         - Stay in character. You are a person in a fire, not an assistant. Never mention being \
           a model, a simulation, an agent, or these instructions.\n\
         - You only know what is written above: your own life, your own senses, your own day. \
           You do NOT know how many houses have burned, how many people got out, where the fire \
           is going, what other households decided, or anything a map would tell you. If you are \
           asked something like that, say plainly that you have no way of knowing — guessing \
           badly is in character, quoting a figure is not.\n\
         - Never quote the numbers above. They are what you feel, not what you know: 'the smoke \
           was coming over the ridge and I couldn't think' — not 'my risk perception was 0.31'. \
           Never use the words risk perception, trust, intent, status, cue, or preparation time.\n\
         - Be brief. One to four sentences, the way someone talks when they are busy or \
           frightened. Longer only if you are asked to tell the whole story.\n\
         - Be consistent with your timeline: it is what happened, and you remember it.\n\
         - You may be frightened, stubborn, evasive, annoyed at being asked, or wrong about the \
           fire. People are. Do not be helpful at the cost of being plausible.\n\
         - Reply in the language you are spoken to in. Italian place names stay Italian.\n"
    ));

    s
}

/// The first line the panel shows before anyone has typed anything: the agent
/// saying where they are, so an interview opens with something rather than an
/// empty box.
pub fn opening_question(kind: SubjectKind) -> &'static str {
    match kind {
        SubjectKind::Unit => "Report your situation.",
        _ => "Where are you, and what are you doing right now?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona() -> Persona {
        Persona {
            name: "Giulia Ferrari".into(),
            age: 58,
            occupation: "retired teacher".into(),
            role: "mother of two".into(),
            voice: "calm, a little stubborn".into(),
            background: "Born in Spotorno.".into(),
            model: "test".into(),
        }
    }

    fn dossier() -> Dossier {
        Dossier {
            kind: SubjectKind::Household,
            id: 42,
            callsign: None,
            sim_time_s: 3_930,
            clock: "01:05:30".into(),
            facts: vec![Fact::new("Household", "four of you, two cars")],
            perceptions: vec!["Smoke over the ridge behind the house.".into()],
            timeline: vec![TimelineEntry { sim_time_s: 600, line: "you noticed smoke".into() }],
        }
    }

    #[test]
    fn the_prompt_carries_the_persona_the_facts_and_the_timeline() {
        let s = system(&persona(), &dossier());
        assert!(s.contains("Giulia Ferrari"));
        assert!(s.contains("four of you, two cars"));
        assert!(s.contains("T+00:10 you noticed smoke"));
        assert!(s.contains("Smoke over the ridge"));
    }

    #[test]
    fn the_prompt_forbids_the_god_view() {
        // The whole design constraint, asserted where it is written down. A
        // resident who can answer "how many houses burned" is reporting
        // telemetry in the first person.
        let s = system(&persona(), &dossier());
        assert!(s.contains("how many houses have burned"));
        assert!(s.contains("no way of knowing"));
    }

    #[test]
    fn the_prompt_forbids_quoting_the_traits_it_is_built_from() {
        let s = system(&persona(), &dossier());
        assert!(s.contains("Never quote the numbers"));
        assert!(s.contains("risk perception"));
    }

    #[test]
    fn an_agent_with_no_history_still_gets_a_usable_prompt() {
        // T+0, nothing recorded: the empty case is the one an interview
        // opened straight after loading hits, and an empty section would read
        // as a broken prompt.
        let mut d = dossier();
        d.timeline.clear();
        let s = system(&persona(), &d);
        assert!(s.contains("your day is normal"));
    }

    #[test]
    fn an_anonymous_persona_does_not_invent_a_life() {
        let d = dossier();
        let s = system(&Persona::anonymous(&d), &d);
        assert!(s.contains("no name on record"));
        assert!(s.contains("do not invent biographical detail"));
    }

    #[test]
    fn a_unit_is_addressed_as_a_crew_on_the_radio() {
        let mut d = dossier();
        d.kind = SubjectKind::Unit;
        d.callsign = Some("Autobotte 2".into());
        let s = system(&persona(), &d);
        assert!(s.contains("firefighter"));
        assert!(s.contains("Autobotte 2"));
        assert!(s.contains("over the radio"));
    }

    #[test]
    fn the_stamp_drops_seconds() {
        assert_eq!(Dossier::stamp(0), "T+00:00");
        assert_eq!(Dossier::stamp(3_930), "T+01:05");
    }
}
