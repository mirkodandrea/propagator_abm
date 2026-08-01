//! Giving a row of the population bake a name and a voice.
//!
//! A household in this model is `size = 4`, `risk_perception = 0.31`,
//! `trust_authority = 0.62`, `intent = wait_and_see`, `prep_time_min = 18`,
//! and a timeline of things that happened to it. Every one of those is a real
//! quantity the simulation acts on; none of them is a person. A [`Persona`] is
//! the thin layer that makes the numbers answerable — a name, an age, what
//! they do, how they talk — and it is generated **once** by the model and then
//! stored, because a household that introduced itself differently every time
//! you opened the panel would be a different household every time.
//!
//! **The traits are the input, not decoration.** The generation prompt is
//! handed the same dossier the interview will be, so the person it invents is
//! consistent with what the model will make them do: a household with
//! `intent = stay_defend` and a high `defensible_space` comes back as someone
//! with a cleared garden and a reason to think they can hold it, and the
//! interview then has something to interview. Generating a persona from
//! nothing and letting the traits contradict it is how you get an agent whose
//! words and behaviour are unrelated — which reads, again, as a bug in the
//! model rather than in the prompt.
//!
//! **Keyed on scenario and subject, never on a run.** See the crate docs: the
//! population is baked, so household 42's traits are the same in every run and
//! the person built on them should be too.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::prompt::Dossier;
use crate::provider::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    pub name: String,
    pub age: u32,
    /// What they do — "retired schoolteacher", "runs a beach kiosk". For a
    /// suppression unit, their role on the crew.
    pub occupation: String,
    /// Their place in the household, or on the crew: "mother of two", "lives
    /// alone with a dog", "second-year volunteer".
    pub role: String,
    /// How they speak, in a phrase: "clipped, practical, hates fuss".
    pub voice: String,
    /// Two or three sentences of history that the traits imply.
    pub background: String,
    /// The model that wrote this, kept so a persona can be told apart from one
    /// written by a different model later.
    #[serde(default)]
    pub model: String,
}

impl Persona {
    /// A one-line identification for the chat window's title bar.
    pub fn headline(&self) -> String {
        format!("{}, {} — {}", self.name, self.age, self.occupation)
    }

    /// The persona used when generation failed but the interview should still
    /// be possible.
    ///
    /// Deliberately anonymous rather than a random invented name: if the model
    /// could not be reached to write a persona, inventing "Marco Rossi" in
    /// Rust would produce someone the interview then has to keep being, with
    /// no record of where they came from. A resident of the right household
    /// with no name is honest about what is missing.
    pub fn anonymous(dossier: &Dossier) -> Persona {
        Persona {
            name: match dossier.kind {
                SubjectKind::Unit => dossier.callsign.clone().unwrap_or_else(|| "the unit".into()),
                _ => "an unnamed resident".to_string(),
            },
            age: 0,
            occupation: String::new(),
            role: String::new(),
            voice: "plain, tired".to_string(),
            background: String::new(),
            model: String::new(),
        }
    }

    pub fn is_anonymous(&self) -> bool {
        self.age == 0 && self.background.is_empty()
    }
}

pub use crate::subject::SubjectKind;

/// The two messages that ask a model to invent one person consistent with a
/// dossier.
///
/// Separate from the interview prompt because it is a different job with a
/// different output: this one is asked for JSON and nothing else, and its
/// answer is stored rather than shown.
pub fn request(dossier: &Dossier) -> Vec<Message> {
    let who = match dossier.kind {
        SubjectKind::Household => {
            "one adult who speaks for a household in a small town on the Ligurian coast \
             (Spotorno, Bergeggi, Noli) during a wildfire"
        }
        SubjectKind::Person => {
            "one person who is out of the house, away from their family, in a small town \
             on the Ligurian coast during a wildfire"
        }
        SubjectKind::Unit => {
            "one Italian firefighter speaking for their crew — a hand crew (squadra), \
             a water tender (autobotte) or an air tanker — working a wildfire on the \
             Ligurian coast"
        }
    };
    let system = format!(
        "You invent a single plausible person from a simulation's own data, for a wildfire \
         training simulator. Invent {who}.\n\n\
         The facts below come from the simulation and are fixed. Everything you invent must be \
         consistent with them: a household that intends to defend its property is someone with a \
         reason to believe they can; a household with a long preparation time is someone with a \
         reason to be slow. Italian names, and a life that fits a coastal Ligurian town of a few \
         thousand people — fishing, tourism, olives, retirees, commuters to Savona.\n\n\
         Reply with a single JSON object and nothing else. No prose, no code fence. Keys:\n\
         - name (string, an Italian full name)\n\
         - age (integer, consistent with any age given in the facts)\n\
         - occupation (string, a few words)\n\
         - role (string, their place in the household or on the crew, a few words)\n\
         - voice (string, how they speak, one short phrase)\n\
         - background (string, two or three sentences)"
    );
    let mut user = String::from("Facts from the simulation:\n");
    for f in &dossier.facts {
        user.push_str(&format!("- {}: {}\n", f.label, f.value));
    }
    if let Some(cs) = &dossier.callsign {
        user.push_str(&format!("- callsign: {cs}\n"));
    }
    vec![Message::system(system), Message::user(user)]
}

/// Parse whatever the model sent back into a [`Persona`].
///
/// Tolerant about the wrapper and strict about the contents. Models fence
/// their JSON, prefix it with "Here is the persona:", or wrap it in an outer
/// object about half the time, and none of that is worth failing an interview
/// over; a missing name is, because an interview with a nameless persona is
/// the failure this whole module exists to prevent.
pub fn parse(text: &str, model: &str) -> Result<Persona> {
    let json = extract_object(text)
        .ok_or_else(|| anyhow!("the model did not return a JSON object: {}", snippet(text)))?;
    let mut persona: Persona = serde_json::from_str(&json).map_err(|e| {
        anyhow!("the model's persona JSON did not fit: {e} (in {})", snippet(&json))
    })?;
    if persona.name.trim().is_empty() {
        return Err(anyhow!("the model returned a persona with no name"));
    }
    persona.model = model.to_string();
    Ok(persona)
}

/// The outermost `{…}` in a blob of text, brace-counted so a nested object
/// does not end the match early.
fn extract_object(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn snippet(text: &str) -> String {
    let t: String = text.chars().take(160).collect();
    t.replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"name":"Giulia Ferrari","age":58,"occupation":"retired teacher",
        "role":"mother of two, lives with her husband","voice":"calm, a little stubborn",
        "background":"Born in Spotorno. Watched the 2003 fires from this same terrace."}"#;

    #[test]
    fn parses_a_plain_object() {
        let p = parse(GOOD, "test-model").unwrap();
        assert_eq!(p.name, "Giulia Ferrari");
        assert_eq!(p.age, 58);
        assert_eq!(p.model, "test-model");
        assert!(!p.is_anonymous());
    }

    #[test]
    fn survives_a_fence_and_a_preamble() {
        // Both are what models actually send, and neither is worth failing an
        // interview over.
        let text = format!("Here is the persona:\n```json\n{GOOD}\n```\nHope that helps!");
        assert_eq!(parse(&text, "m").unwrap().name, "Giulia Ferrari");
    }

    #[test]
    fn a_nested_object_does_not_end_the_match_early() {
        let text = r#"{"name":"Marco Bruno","age":41,"occupation":"fisherman","role":"father",
            "voice":"blunt","background":"x","extra":{"boat":"Santa Rita"}}"#;
        let p = parse(text, "m").unwrap();
        assert_eq!(p.name, "Marco Bruno");
    }

    #[test]
    fn a_brace_inside_a_string_does_not_end_the_match_early() {
        let text = r#"{"name":"Ada Neri","age":33,"occupation":"barista","role":"lives alone",
            "voice":"quick","background":"Says the town is a \"{}\" of tourists in August."}"#;
        assert_eq!(parse(text, "m").unwrap().name, "Ada Neri");
    }

    #[test]
    fn a_nameless_persona_is_refused() {
        let text = r#"{"name":"  ","age":40,"occupation":"x","role":"y","voice":"z","background":"w"}"#;
        assert!(parse(text, "m").is_err());
    }

    #[test]
    fn prose_with_no_object_is_refused_with_the_text_in_the_error() {
        let e = parse("I'm sorry, I can't help with that.", "m").unwrap_err();
        assert!(e.to_string().contains("I'm sorry"));
    }
}
