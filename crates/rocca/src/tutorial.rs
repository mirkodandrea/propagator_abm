//! The tutorial (docs/01-GIOCO.md §1): a small fire, played by the player one
//! prompted step at a time. Each step accepts only the action it asks for, so
//! the orders, and with them the outcome, are scripted; the fire and the
//! teams are the real simulation. The kiosk shows the text and refuses any
//! other action; the headless test plays the same script
//! ([`Script::play`]).

use anyhow::Result;

use crate::hex::Axial;
use crate::Game;

/// Simulated minutes the tutorial runs before it pauses by itself: at ×60
/// that is 30 real seconds (user, 2026-10-14).
pub const RUN_MIN: i64 = 30;

/// What a step waits for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expect {
    /// «Avanti».
    Next,
    /// The pause button. No script step asks for it: the tutorial opens
    /// and stops by itself, and the text says the player will have to do it.
    Pause,
    /// Selecting team `t` (a click on its flag). No script step asks for it
    /// any more: a `Move` step accepts it for its team.
    Select(usize),
    /// Dropping team `t`'s flag on hex `at`: the order is given.
    Move(usize, Axial),
    /// Choosing district `d` for evacuation, then confirming.
    Evacuate(usize),
    /// Ending the pause.
    Resume,
    /// The world runs by itself until the game clock reaches this many
    /// simulated seconds, then the game pauses on its own.
    Run(i64),
    /// The evacuation is finished (or the fire is out and nobody is on the
    /// road): the tutorial ends and the game goes on.
    Over,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub text: String,
    pub expect: Expect,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub steps: Vec<Step>,
}

impl Script {
    /// The script for the tutorial `game` (built from `game.json`'s
    /// `tutorial`). `None` if the game has no tutorial or its places are
    /// missing.
    pub fn of(game: &Game) -> Option<Script> {
        let tu = game.territory.tutorial.as_ref()?;
        let d = game.district_index(&tu.evacuate)?;
        let name = game.districts[d].name.clone();
        let mut steps = vec![
            Step { text: format!("Giorno 1. Un piccolo incendio è partito vicino a {name}. Il gioco comincia in pausa: il tempo è fermo e puoi dare gli ordini."), expect: Expect::Next },
            Step {
                text: "Le aree colorate sono dove il fuoco può arrivare tra 1 ora e tra 3 ore, senza interventi. In alto il vento previsto.".into(),
                expect: Expect::Next,
            },
        ];
        for &(t, p) in &tu.moves {
            let hex = game.hexes.nearest(scenario::Pos::from(p))?;
            let team = &game.teams[t];
            let who = if team.volunteers { "volontari" } else { "vigili" };
            let from = game.territory.stations[team.station].name.rsplit(" - ").next().unwrap_or_default().to_string();
            steps.push(Step {
                text: format!(
                    "Manda {} da {from}: trascina la sua bandiera (un'autobotte e {} {who}) sull'esagono indicato. Mentre la trascini vedi il percorso e il tempo di viaggio. Lasciala lì: la squadra parte.",
                    team.name, team.crew
                ),
                expect: Expect::Move(t, hex.at),
            });
        }
        let t0 = game.time_s();
        steps.push(Step { text: "Gli ordini sono dati. Premi «Riprendi» (in alto a destra): il tempo torna a scorrere.".into(), expect: Expect::Resume });
        steps.push(Step {
            text: "Guarda le squadre arrivare e il fuoco avanzare: passerà mezz'ora di gioco, circa 30 secondi.".into(),
            expect: Expect::Run(t0 + RUN_MIN * 60),
        });
        steps.push(Step {
            text: format!("Il gioco si è fermato da solo, solo per questa prova. Nella partita vera dovrai fermarlo tu, premendo «Pausa»: vedrai anche le previsioni di dove andrà il fuoco nelle prossime ore, e potrai rimetterla dopo 30 secondi. Ora le persone: passa sopra {name}, vedi il percorso verso l'area sicura e quanto ci vorrà. Poi clicca {name} e premi «Evacua»."),
            expect: Expect::Evacuate(d),
        });
        steps.push(Step { text: "Premi «Riprendi»: il tempo torna a scorrere finché l'evacuazione non è finita.".into(), expect: Expect::Resume });
        steps.push(Step { text: "Guarda le persone lasciare le case e mettersi al sicuro: il tutorial finisce quando l'evacuazione è terminata.".into(), expect: Expect::Over });
        Some(Script { steps })
    }

    /// Play the script headless, as a player who does exactly what each step
    /// asks; returns the finished game.
    pub fn play(&self, game: &mut Game) -> Result<()> {
        game.opening_pause();
        for s in &self.steps {
            match s.expect {
                Expect::Next | Expect::Select(_) => {}
                Expect::Pause => game.pause().map_err(anyhow::Error::msg)?,
                Expect::Move(t, at) => {
                    game.move_team(t, at).map_err(anyhow::Error::msg)?;
                }
                Expect::Evacuate(d) => {
                    game.evacuate(d).map_err(anyhow::Error::msg)?;
                }
                Expect::Resume => game.resume(),
                Expect::Run(t) => {
                    game.run_until(t)?;
                    game.opening_pause();
                }
                Expect::Over => {
                    while !game.is_over() && !game.evacuation_finished() {
                        game.step()?;
                    }
                }
            }
        }
        Ok(())
    }
}
