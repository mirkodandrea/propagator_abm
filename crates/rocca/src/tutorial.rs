//! The tutorial (docs/01-GIOCO.md §1): a small fire, played by the player one
//! prompted step at a time. Each step accepts only the action it asks for, so
//! the orders, and with them the outcome, are scripted; the fire and the
//! teams are the real simulation. The kiosk shows the text and refuses any
//! other action; the headless test plays the same script
//! ([`Script::play`]).

use anyhow::Result;

use crate::hex::Axial;
use crate::Game;

/// Simulated minutes the tutorial runs before it pauses by itself: at ×120
/// that is 15 real seconds (user, 2026-10-14).
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
            Step { text: format!("Giorno 1. Un incendio è partito vicino a __{name}__. Il tempo è **fermo**: ora puoi dare gli ordini."), expect: Expect::Next },
            Step {
                text: "Le linee rosse mostrano dove il fuoco può arrivare **tra 1 e 3 ore**, se nessuno interviene. In basso a destra vedi le __condizioni__: il vento e l'umidità.".into(),
                expect: Expect::Next,
            },
            Step {
                text: "L'anello accanto al nome di ogni __borgo__ indica la sua **vita**: si svuota quando il fuoco raggiunge le case. Passa da **verde** a **giallo**, poi a **rosso**, man mano che le case rimaste intatte diminuiscono.".into(),
                expect: Expect::Next,
            },
        ];
        for &(t, p) in &tu.moves {
            let hex = game.hexes.nearest(scenario::Pos::from(p))?;
            let team = &game.teams[t];
            let from = game.territory.stations[team.station].name.rsplit(" - ").next().unwrap_or_default().to_string();
            steps.push(Step {
                text: format!(
                    "La {} parte da __{from}__ ({} persone). **Trascina la squadra** sull'esagono indicato e lascialo lì: vedi il percorso e il tempo di viaggio.",
                    team.name, team.crew
                ),
                expect: Expect::Move(t, hex.at),
            });
        }
        steps.push(Step {
            text: "Il contorno **giallo** mostra la zona che le squadre schierate possono coprire.".into(),
            expect: Expect::Next,
        });
        let t0 = game.time_s();
        steps.push(Step { text: "Ordini dati. Premi **Riprendi** (in basso al centro): il tempo riparte.".into(), expect: Expect::Resume });
        steps.push(Step {
            text: format!("Guarda le squadre arrivare e il fuoco avanzare: passano **{RUN_MIN} minuti** di gioco, circa {} secondi.", RUN_MIN * 60 / crate::RUN_X as i64),
            expect: Expect::Run(t0 + RUN_MIN * 60),
        });
        steps.push(Step {
            text: format!("Il gioco si è fermato da solo. Nella partita vera premi tu **Pausa**: la mappa mostra le previsioni e puoi dare ordini.\n\nOra le persone: clicca __{name}__, poi premi **Evacua**."),
            expect: Expect::Evacuate(d),
        });
        steps.push(Step { text: "Premi **Riprendi**: il tempo scorre finché l'evacuazione non è finita.".into(), expect: Expect::Resume });
        steps.push(Step { text: "Guarda le persone mettersi al sicuro. Il tutorial finisce **a evacuazione completata**.".into(), expect: Expect::Over });
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
