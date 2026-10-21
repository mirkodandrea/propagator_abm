//! The stars and the day's board (docs/01-GIOCO.md §5, user 2026-10-14):
//! people saved first, then homes, then hectares.
//!
//! Every star is a share against the same fire played with no orders
//! ([`crate::Game::without_orders`]): of the people it would catch, the share
//! the player spared; the same for the homes; and the share of its burnt
//! hectares the player kept from burning. Shares, not counts, so a small
//! fire played well beats a large one played badly. Nothing here reads the
//! simulation: it only weighs two [`Outcome`]s.

use crate::tr;
use serde::{Deserialize, Serialize};

use crate::Outcome;

/// Persone, Case, Ettari: 1 to 5 stars each; `None` where the fire put
/// nothing at stake (no person or home at risk with no orders).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stars {
    pub persone: Option<u8>,
    pub case: Option<u8>,
    pub ettari: Option<u8>,
}

/// The share each further star needs. Provisional (2026-10-14): to be set
/// from the headless trials (docs/03-VERIFICHE.md).
pub const PERSONE: [f64; 4] = [0.2, 0.45, 0.7, 0.9];
pub const CASE: [f64; 4] = [0.1, 0.3, 0.5, 0.75];
pub const ETTARI: [f64; 4] = [0.1, 0.25, 0.45, 0.65];

/// 1 to 5 stars for a share of 0-1: one more for each threshold it reaches.
pub fn stars_of(share: f64, thresholds: [f64; 4]) -> u8 {
    1 + thresholds.iter().filter(|&&t| share >= t).count() as u8
}

/// The player's game against the same fire with no orders.
pub fn stars(mine: &Outcome, without: &Outcome) -> Stars {
    let spared = |risk: f64, lost: f64, t| (risk > 0.0).then(|| stars_of(((risk - lost) / risk).max(0.0), t));
    Stars {
        persone: spared(without.caught() as f64, mine.caught() as f64, PERSONE),
        case: spared(without.homes_hit() as f64, mine.homes_hit() as f64, CASE),
        ettari: spared(without.hectares as f64, mine.hectares as f64, ETTARI),
    }
}

/// Candidate scale for review, deliberately separate from the live board.
/// 10/25/50/75% of people, 5/15/30/50% of homes, 5/10/20/35% of land spared.
pub fn proposed_stars(mine: &Outcome, without: &Outcome) -> Stars {
    let spared = |risk: f64, lost: f64, thresholds| {
        (risk > 0.0).then(|| stars_of(((risk - lost) / risk).max(0.0), thresholds))
    };
    Stars {
        persone: spared(without.caught() as f64, mine.caught() as f64, [0.10, 0.25, 0.50, 0.75]),
        case: spared(without.homes_hit() as f64, mine.homes_hit() as f64, [0.05, 0.15, 0.30, 0.50]),
        ettari: spared(without.hectares as f64, mine.hectares as f64, [0.05, 0.10, 0.20, 0.35]),
    }
}

/// A factual debrief, shared by desktop and mobile. Outcomes establish the
/// difference, not the cause: never infer which order was early or late.
pub struct Debrief {
    pub results: [String; 3],
    pub next_try: &'static str,
}

pub fn debrief(mine: &Outcome, without: &Outcome) -> Debrief {
    fn result(label: &str, lost: f64, risk: f64, unit: &str) -> String {
        if risk <= 0.0 {
            return if lost > 0.0 {
                tr!("{0}: il fuoco ha colpito {1} {2}, mentre senza ordini nessuno sarebbe stato colpito.", label, format!("{lost:.0}"), unit)
            } else {
                tr!("{0}: qui non ci sono stati danni, nemmeno senza ordini. Questa voce non assegna stelle.", label)
            };
        }
        let difference = risk - lost;
        if difference < 0.0 {
            tr!("{0}: il fuoco ha colpito {1} {2} in più rispetto alla partita senza ordini.", label, format!("{:.0}", -difference), unit)
        } else if difference == 0.0 {
            tr!("{0}: il fuoco ha colpito {1} {2}, lo stesso danno della partita senza ordini.", label, format!("{lost:.0}"), unit)
        } else {
            let percent = 100.0 * difference / risk;
            tr!("{0}: il fuoco ha colpito {1} {2} in meno. Hai evitato il {3}% del danno.", label, format!("{difference:.0}"), unit, format!("{percent:.0}"))
        }
    }
    let next_try = if mine.caught() > 0 {
        tr!("La prossima volta, guarda quanto richiede l'evacuazione e dove potrebbe arrivare il fuoco. Metti al primo posto le persone.")
    } else if mine.districts.iter().any(|d| d.on_the_road > 0) {
        tr!("Il fuoco non ha raggiunto nessuno, ma alcune persone sono ancora in viaggio: devono ancora arrivare al sicuro.")
    } else if mine.units_lost > 0 {
        tr!("Anche i soccorritori hanno bisogno di protezione. Prima di spostarli, controlla il percorso e la previsione.")
    } else if mine.homes_hit() > 0 {
        tr!("La prossima volta, prova a difendere i borghi minacciati ricordando che anche le squadre hanno bisogno di tempo per arrivare.")
    } else {
        tr!("Prova a cambiare il momento degli ordini e osserva cosa succede. La previsione ti aiuta, ma non può dirti tutto con certezza.")
    };
    Debrief {
        results: [
            result(tr!("Persone"), mine.caught() as f64, without.caught() as f64, tr!("abitanti")),
            result(tr!("Case"), mine.homes_hit() as f64, without.homes_hit() as f64, tr!("edifici")),
            result(tr!("Bosco"), mine.hectares as f64, without.hectares as f64, tr!("ettari")),
        ],
        next_try,
    }
}

/// Offline-readable civic advice, paraphrased from the official Io non rischio
/// wildfire guidance. These are citizen actions, not simulated crew tactics.
pub fn civil_protection() -> [(&'static str, &'static str); 3] {
    [
        (tr!("Preparati prima"), tr!("Sai cosa fare se un incendio si avvicina? Informati sul Piano di protezione civile del tuo Comune: meglio prepararsi prima.")),
        (tr!("Segnala fumo e fiamme"), tr!("Se vedi fumo o fiamme, chiama subito il 115 o, dove attivo, il 112. Spiega bene dove sei: non aspettare che chiami qualcun altro.")),
        (tr!("Lascia passare i soccorsi"), tr!("Lascia libere le strade per chi sta intervenendo. Segui le indicazioni delle autorità e, se ti chiedono di evacuare, parti subito.")),
    ]
}
pub const CIVIL_PROTECTION_SOURCE: &str = "https://iononrischio.protezionecivile.gov.it/it/preparati/incendi-boschivi/cosa-fare/";

impl Stars {
    /// Persone, Case, Ettari.
    pub fn all(&self) -> [Option<u8>; 3] {
        [self.persone, self.case, self.ettari]
    }
}

/// One finished game on the day's board: the name, the level (stars are
/// not comparable between fires), the stars and when it was played.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Played {
    pub name: String,
    pub level: String,
    /// Persone, Case, Ettari; `None` where nothing was at stake.
    pub stars: [Option<u8>; 3],
    /// The local date, `YYYY-MM-DD`: the board shows only today's games.
    #[serde(default)]
    pub day: String,
    /// Seconds since the Unix epoch: the earlier game wins a tie.
    #[serde(default)]
    pub at: u64,
}

impl Played {
    /// The ranking key: Persone, then Case, then Ettari stars (a category
    /// with nothing at stake counts as full: every player of a fire faces
    /// the same one).
    pub fn key(&self) -> [u8; 3] {
        self.stars.map(|s| s.unwrap_or(5))
    }
}

/// Games kept per level and day (a kiosk day never gets near it).
pub const BOARD_KEEP: usize = 50;
/// Rows the screens show per level.
pub const BOARD_SHOWN: usize = 8;

/// The best of the day, per level. Ranking: Persone stars, then Case, then
/// Ettari, then the earlier game.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub entries: Vec<Played>,
}

impl Board {
    /// Add a game. Games of other days are dropped, the board being «del
    /// giorno»; each level keeps its best [`BOARD_KEEP`].
    pub fn push(&mut self, p: Played) {
        let day = p.day.clone();
        self.entries.retain(|e| e.day == day);
        self.entries.push(p);
        let mut levels: Vec<String> = self.entries.iter().map(|e| e.level.clone()).collect();
        levels.sort();
        levels.dedup();
        let mut kept = Vec::new();
        for l in levels {
            kept.extend(self.ranking(&l, &day).into_iter().take(BOARD_KEEP).cloned());
        }
        self.entries = kept;
    }

    /// Today's games at `level`, best first.
    pub fn ranking(&self, level: &str, day: &str) -> Vec<&Played> {
        let mut v: Vec<&Played> = self.entries.iter().filter(|e| e.level == level && e.day == day).collect();
        v.sort_by(|a, b| b.key().cmp(&a.key()).then(a.at.cmp(&b.at)));
        v
    }

    /// Today's games at `level`.
    pub fn count(&self, level: &str, day: &str) -> usize {
        self.entries.iter().filter(|e| e.level == level && e.day == day).count()
    }

    /// The 0-based place of the game played at `at` by `name` in today's
    /// ranking of its level, if it is there.
    pub fn place(&self, level: &str, day: &str, name: &str, at: u64) -> Option<usize> {
        self.ranking(level, day).iter().position(|e| e.at == at && e.name == name)
    }

    /// Read a saved board; anything unreadable is an empty board, so a
    /// damaged file never stops the kiosk.
    pub fn from_json(s: &str) -> Board {
        serde_json::from_str(s).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Longest name kept, in characters.
pub const NAME_MAX: usize = 16;

/// The name as typed, tidied: no spaces around it, at most [`NAME_MAX`]
/// characters, first letter in capital. Empty if nothing was typed.
pub fn name(typed: &str) -> String {
    let t: String = typed.trim().chars().take(NAME_MAX).collect();
    let mut c = t.trim_end().chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DistrictOutcome;

    fn outcome(caught: u32, homes_hit: usize, hectares: f32) -> Outcome {
        let d = DistrictOutcome { caught, homes_hit, ..Default::default() };
        Outcome { at_s: 0, districts: vec![d], hectares, units_lost: 0 }
    }

    #[test]
    fn stars_are_shares_spared() {
        let none = outcome(40, 20, 300.0);
        assert_eq!(stars(&none, &none).all(), [Some(1), Some(1), Some(1)]);
        assert_eq!(stars(&outcome(0, 0, 10.0), &none).all(), [Some(5), Some(5), Some(5)]);
        // worse than no orders: still one star
        assert_eq!(stars(&outcome(50, 30, 400.0), &none).persone, Some(1));
        // nothing at stake
        assert_eq!(stars(&outcome(0, 0, 10.0), &outcome(0, 0, 50.0)).all(), [None, None, Some(5)]);
    }

    #[test]
    fn proposed_scale_rewards_progress_without_rewarding_inaction() {
        let base = outcome(100, 100, 100.0);
        assert_eq!(proposed_stars(&base, &base).all(), [Some(1); 3]);
        assert_eq!(proposed_stars(&outcome(50, 50, 50.0), &base).all(), [Some(4), Some(5), Some(5)]);
        assert_eq!(proposed_stars(&outcome(0, 0, 0.0), &outcome(0, 0, 0.0)).all(), [None; 3]);
        let mut previous = [1; 3];
        for saved in 0..=100 {
            let mine = outcome(100 - saved, (100 - saved) as usize, (100 - saved) as f32);
            let live = stars(&mine, &base).all();
            let candidate = proposed_stars(&mine, &base).all();
            for i in 0..3 {
                let value = candidate[i].unwrap();
                assert!(value >= previous[i] && value >= live[i].unwrap());
                previous[i] = value;
            }
        }
    }

    #[test]
    fn feedback_explains_progress_regressions_and_no_risk() {
        let base = outcome(100, 20, 200.0);
        let report = debrief(&outcome(50, 10, 100.0), &base);
        assert!(report.results[0].contains("50%"));
        assert!(report.next_try.contains("evacuazione"));
        assert!(debrief(&outcome(110, 21, 201.0), &base).results[0].contains("in più"));
        assert!(debrief(&base, &base).results[1].contains("stesso danno"));
        let zero = outcome(0, 0, 0.0);
        assert!(debrief(&zero, &zero).results[0].contains("non assegna stelle"));
        assert!(debrief(&base, &zero).results[0].contains("senza ordini nessuno"));
        let mut unfinished = zero.clone();
        unfinished.districts[0].on_the_road = 12;
        assert!(debrief(&unfinished, &base).next_try.contains("ancora in viaggio"));
        let mut crew_lost = zero.clone();
        crew_lost.units_lost = 1;
        assert!(debrief(&crew_lost, &base).next_try.contains("soccorritori"));
    }

    fn game(name: &str, stars: [u8; 3], day: &str, at: u64) -> Played {
        Played { name: name.into(), level: "Gioco".into(), stars: stars.map(Some), day: day.into(), at }
    }

    #[test]
    fn the_board_ranks_people_then_homes_then_hectares() {
        let mut b = Board::default();
        b.push(game("Ieri", [5, 5, 5], "2026-10-13", 1));
        b.push(game("Anna", [4, 2, 5], "2026-10-14", 100));
        b.push(game("Bruno", [4, 3, 1], "2026-10-14", 200));
        b.push(game("Carla", [5, 1, 1], "2026-10-14", 300));
        b.push(game("Dario", [4, 2, 5], "2026-10-14", 50));
        let names: Vec<_> = b.ranking("Gioco", "2026-10-14").iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, ["Carla", "Bruno", "Dario", "Anna"]);
        assert!(b.entries.iter().all(|e| e.day == "2026-10-14"));
        assert_eq!(Board::from_json(&b.to_json()), b);
        assert_eq!(Board::from_json("not json"), Board::default());
    }

    #[test]
    fn a_name_is_tidied() {
        assert_eq!(name("  giulia "), "Giulia");
        assert_eq!(name("   "), "");
        assert_eq!(name("élodie maria rossi bianchi"), "Élodie maria ros");
    }
}
