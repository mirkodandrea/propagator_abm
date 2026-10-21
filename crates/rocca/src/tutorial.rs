//! The tutorial (docs/01-GIOCO.md §1): a small fire, played by the player one
//! prompted step at a time. Each step accepts only the action it asks for, so
//! the orders, and with them the outcome, are scripted; the fire and the
//! teams are the real simulation. The kiosk shows the text and refuses any
//! other action; the headless test plays the same script
//! ([`Script::play`]).

use crate::tr;
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
    /// The same step for a touch screen (nothing to click, no Esc key).
    pub touch: String,
    pub expect: Expect,
}

impl Step {
    /// A step that reads the same with a mouse and with a finger.
    fn same(text: String, expect: Expect) -> Step {
        Step { touch: text.clone(), text, expect }
    }
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
            Step::same(tr!("Un incendio è partito vicino a __{0}__. Il tempo è **fermo**: ora puoi dare gli ordini.", name), Expect::Next),
            Step {
                text: tr!("Le linee rosse mostrano dove il fuoco può arrivare **tra 1 e 3 ore**, se nessuno interviene. In basso a destra vedi le __condizioni__: il vento e l'umidità.").into(),
                touch: tr!("Le linee rosse mostrano dove il fuoco può arrivare **tra 1 e 3 ore**, se nessuno interviene. In alto vedi le __condizioni__: il vento e l'umidità.").into(),
                expect: Expect::Next,
            },
            Step::same(
                tr!("L'anello accanto al nome di ogni __borgo__ indica la sua **vita**: si svuota quando il fuoco raggiunge le case, e il cartellino passa da **blu** a **giallo**, poi a **rosso**. Un borgo **evacuato** diventa **verde**. Da lontano i borghi tranquilli si riducono a un anello: passaci sopra per leggere il nome.").into(),
                Expect::Next,
            ),
            Step {
                text: tr!("Se qualcosa va storto **fuori dalla vista** (un borgo perde case, una squadra è in pericolo) compare una **freccia rossa** sul bordo dello schermo: **cliccala** e la mappa ti ci porta.").into(),
                touch: tr!("Se qualcosa va storto **fuori dalla vista** (un borgo perde case, una squadra è in pericolo) compare una **freccia rossa** sul bordo dello schermo: **toccala** e la mappa ti ci porta.").into(),
                expect: Expect::Next,
            },
        ];
        for (i, &(t, p)) in tu.moves.iter().enumerate() {
            let hex = game.hexes.nearest(scenario::Pos::from(p))?;
            let team = &game.teams[t];
            let from = game.territory.stations[team.station].name.rsplit(" - ").next().unwrap_or_default().to_string();
            let (how, how_touch) = if i == 0 {
                (
                    tr!("Ogni squadra ha il suo **colore**. **Trascina l'icona** sulla destinazione indicata, oppure **clicca la squadra** e poi la destinazione: vedi strada e tempo prima di decidere. **Esc** annulla."),
                    tr!("Ogni squadra ha il suo **colore**. **Trascina l'icona** sulla destinazione indicata, oppure **tocca la squadra** e poi la destinazione: vedi strada e tempo prima di decidere."),
                )
            } else {
                (
                    tr!("Anche stavolta: trascina l'icona sulla destinazione indicata, o cliccala e poi clicca la destinazione."),
                    tr!("Anche stavolta: trascina l'icona sulla destinazione indicata, o toccala e poi tocca la destinazione."),
                )
            };
            steps.push(Step {
                text: tr!("La {0} parte da __{1}__ ({2} persone). {3}", team.name, from, team.crew, how),
                touch: tr!("La {0} parte da __{1}__ ({2} persone). {3}", team.name, from, team.crew, how_touch),
                expect: Expect::Move(t, hex.at),
            });
        }
        steps.push(Step::same(
            tr!("Il contorno **del colore di ogni squadra** mostra la zona che può coprire. Una squadra che il fuoco minaccia **lampeggia in rosso** e si ritira da sola.").into(),
            Expect::Next,
        ));
        let t0 = game.time_s();
        steps.push(Step::same(tr!("Ordini dati. Premi **Riprendi** (in basso al centro): il tempo riparte.").into(), Expect::Resume));
        steps.push(Step::same(
            tr!("Guarda le squadre arrivare e il fuoco avanzare: passano **{0} minuti** di gioco, circa {1} secondi.", RUN_MIN, RUN_MIN * 60 / crate::RUN_X as i64),
            Expect::Run(t0 + RUN_MIN * 60),
        ));
        steps.push(Step {
            text: tr!("Il gioco si è fermato da solo. Nella partita vera il tempo si ferma anche **cliccando un borgo o una squadra**, oppure con **Pausa**: la mappa mostra le previsioni e puoi dare ordini.\n\nOra le persone: clicca __{0}__, poi premi **Evacua**.", name),
            touch: tr!("Il gioco si è fermato da solo. Nella partita vera il tempo si ferma anche **toccando un borgo o una squadra**, oppure con **Pausa**: la mappa mostra le previsioni e puoi dare ordini.\n\nOra le persone: tocca __{0}__, poi premi **Evacua**.", name),
            expect: Expect::Evacuate(d),
        });
        steps.push(Step::same(tr!("Premi **Riprendi**: il tempo scorre finché l'evacuazione non è finita.").into(), Expect::Resume));
        steps.push(Step::same(tr!("Guarda le persone mettersi al sicuro. Il tutorial finisce **a evacuazione completata**.").into(), Expect::Over));
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
