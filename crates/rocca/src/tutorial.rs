//! The tutorial (docs/01-GIOCO.md §1): a small fire, played by the player one
//! prompted step at a time. Each step accepts only the action it asks for, so
//! the orders, and with them the outcome, are scripted; the fire and the
//! teams are the real simulation. The kiosk shows the text and refuses any
//! other action; the headless test plays the same script
//! ([`Script::play`]).

use anyhow::Result;

use crate::hex::Axial;
use crate::Game;

/// What a step waits for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expect {
    /// «Avanti».
    Next,
    /// The pause button.
    Pause,
    /// Selecting team `t` (a click on its icon).
    Select(usize),
    /// Choosing hex `at` for team `t`, then confirming.
    Move(usize, Axial),
    /// Choosing district `d` for evacuation, then confirming.
    Evacuate(usize),
    /// Ending the pause.
    Resume,
    /// The fire is out and nobody is on the road: the game goes on by itself.
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
            Step { text: format!("Giorno 1. Un piccolo incendio è partito vicino a {name}. Il tempo scorre: il fuoco non aspetta."), expect: Expect::Next },
            Step {
                text: "Premi «Pausa»: chiedi all'esperto dove andrà il fuoco. In pausa il tempo rallenta e puoi dare ordini.".into(),
                expect: Expect::Pause,
            },
            Step {
                text: "Le aree colorate sono dove il fuoco può arrivare tra 1 ora e tra 3 ore, senza interventi. In alto il vento previsto.".into(),
                expect: Expect::Next,
            },
        ];
        for &(t, p) in &tu.moves {
            let hex = game.hexes.nearest(scenario::Pos::from(p))?;
            let team = game.teams.get(t)?.name.clone();
            steps.push(Step { text: format!("Clicca la squadra {team}: un'autobotte e {} vigili.", game.teams[t].crew), expect: Expect::Select(t) });
            steps.push(Step {
                text: "Clicca l'esagono indicato vicino al fuoco: vedi il percorso e il tempo di viaggio. Poi premi «Conferma».".into(),
                expect: Expect::Move(t, hex.at),
            });
        }
        steps.push(Step {
            text: format!("Ora le persone. Clicca {name} e premi «Evacua»: vedi il percorso verso l'area sicura e quanto ci vorrà. Poi «Conferma»."),
            expect: Expect::Evacuate(d),
        });
        steps.push(Step { text: "Premi «Riprendi»: il tempo torna veloce e le squadre si mettono al lavoro.".into(), expect: Expect::Resume });
        steps.push(Step { text: "Guarda le squadre spegnere il fuoco e le persone mettersi al sicuro.".into(), expect: Expect::Over });
        Some(Script { steps })
    }

    /// Play the script headless, as a player who does exactly what each step
    /// asks; returns the finished game.
    pub fn play(&self, game: &mut Game) -> Result<()> {
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
                Expect::Over => {
                    while !game.is_over() {
                        game.step()?;
                    }
                }
            }
        }
        Ok(())
    }
}
