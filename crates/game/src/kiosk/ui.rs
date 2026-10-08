//! The kiosk's panels, provisional until the phase-5 UX: the player's places
//! in rank order with the civil orders, the coordinator's answer, the clock,
//! the outcome, and the operator's new-game control. Reads the game and the
//! plan being composed; every change goes through `kiosk::commit` or
//! `kiosk::new_game`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use rocca::Civil;

use super::{close_crisis, commit, new_game, Kiosk, Phase, CRISIS_S};
use crate::sim::{Sim, SimRestarted};

fn clock(s: i64) -> String {
    format!("T+{}:{:02}", s / 3600, (s / 60) % 60)
}

fn compass(deg: f64) -> &'static str {
    const N: [&str; 8] = ["N", "NE", "E", "SE", "S", "SO", "O", "NO"];
    N[(((deg + 22.5).rem_euclid(360.0)) / 45.0) as usize % 8]
}

pub fn draw(
    mut contexts: EguiContexts,
    mut kiosk: ResMut<Kiosk>,
    mut sim: ResMut<Sim>,
    mut focus: ResMut<crate::ui::UiFocus>,
    mut restarted: EventWriter<SimRestarted>,
) {
    let ctx = contexts.ctx_mut();
    let k = &mut *kiosk;

    egui::TopBottomPanel::top("barra").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading(super::TITLE);
            ui.separator();
            let w = sim.fire.weather();
            ui.label(format!("{}  ·  vento da {} {:.0} km/h  ·  {}", sim.case.name, compass(w.wind_dir_deg), w.wind_speed_kmh, clock(sim.time_s())));
            ui.separator();
            ui.label(match k.phase {
                Phase::Pianifica => "Pianifica (tempo fermo)".to_string(),
                Phase::Esegui => format!("Esegui (×{:.0})", k.speed),
                Phase::Crisi => format!("CRISI (×1): {:.0} s per decidere", (CRISIS_S - k.phase_t).max(0.0)),
                Phase::Fine => "Fine".to_string(),
            });
        });
    });

    if let (Phase::Crisi, Some(c)) = (k.phase, &k.crisis) {
        egui::TopBottomPanel::top("crisi").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(egui::Color32::from_rgb(230, 120, 40), egui::RichText::new("Crisi").heading());
                ui.label(egui::RichText::new(&c.text).size(18.0));
                ui.label(format!("Il fuoco non si ferma. Hai {:.0} s: cambia priorità o ordini, poi Conferma. Se non fai nulla resta il piano attuale.", (CRISIS_S - k.phase_t).max(0.0)));
            });
        });
    }

    egui::SidePanel::left("piano").min_width(320.0).show(ctx, |ui| {
        ui.heading("Priorità");
        ui.label("Ordina i luoghi da difendere. Il coordinatore sceglie dove mettere i mezzi.");
        let n = sim.districts.len();
        let mut order: Vec<usize> = k.proposed.priorities.clone();
        let mut changed = false;
        for (r, &d) in order.clone().iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}. {}", r + 1, sim.districts[d].name));
                if r > 0 && ui.small_button("su").clicked() {
                    order.swap(r, r - 1);
                    changed = true;
                }
                if r + 1 < order.len() && ui.small_button("giù").clicked() {
                    order.swap(r, r + 1);
                    changed = true;
                }
                if ui.small_button("togli").clicked() {
                    order.retain(|&x| x != d);
                    changed = true;
                }
            });
        }
        let unranked: Vec<usize> = (0..n).filter(|d| !order.contains(d)).collect();
        for d in unranked {
            if ui.button(format!("+ Difendi {}", sim.districts[d].name)).clicked() {
                order.push(d);
                changed = true;
            }
        }
        ui.separator();
        ui.heading("Popolazione");
        for d in 0..n {
            let active = sim.active.civil[d];
            ui.horizontal(|ui| {
                ui.label(&sim.districts[d].name);
                for (c, label) in [(Civil::Preallerta, "Preallerta"), (Civil::Evacua, "Evacua")] {
                    let on = k.proposed.civil[d] >= c;
                    // orders already given cannot be taken back
                    let locked = active >= c;
                    if ui.add_enabled(!locked, egui::SelectableLabel::new(on, label)).clicked() {
                        k.proposed.civil[d] = if on { active.max(Civil::Nessuno) } else { c };
                        changed = true;
                    }
                }
            });
        }
        if changed {
            k.proposed.priorities = order;
            k.dirty = true;
        }
        ui.separator();
        let pending = k.proposed != sim.active;
        match k.phase {
            Phase::Pianifica => {
                if ui.button(egui::RichText::new("Conferma e avvia").heading()).clicked() {
                    commit(k, &mut sim);
                    k.enter(Phase::Esegui);
                }
            }
            Phase::Esegui => {
                if ui.add_enabled(pending, egui::Button::new(egui::RichText::new("Conferma il nuovo piano").heading())).clicked() {
                    commit(k, &mut sim);
                }
            }
            Phase::Crisi => {
                if ui.button(egui::RichText::new("Conferma").heading()).clicked() {
                    close_crisis(k, &mut sim);
                }
            }
            Phase::Fine => {}
        }

        ui.separator();
        ui.heading("Piano proposto");
        if let Some(p) = &k.preview {
            for post in p.posts.iter().flatten() {
                ui.label(&post.reason);
            }
            for (_, why) in &p.uncovered {
                ui.colored_label(egui::Color32::from_rgb(200, 120, 40), format!("Scoperto: {why}"));
            }
            for &d in &p.quiet {
                ui.label(format!("{}: il fuoco non lo minaccia ora", sim.districts[d].name));
            }
        }
    });

    egui::SidePanel::right("registro").min_width(300.0).show(ctx, |ui| {
        if k.phase == Phase::Fine {
            ui.heading("Com'è andata");
            let o = sim.outcome();
            egui::Grid::new("esito").striped(true).show(ui, |ui| {
                for h in ["", "famiglie", "case colpite", "colte in casa", "evacuate"] {
                    ui.strong(h);
                }
                ui.end_row();
                for (d, x) in sim.districts.iter().zip(&o.districts) {
                    ui.label(&d.name);
                    ui.label(x.households.to_string());
                    ui.label(x.homes_hit.to_string());
                    ui.label(x.caught.to_string());
                    ui.label(x.evacuated.to_string());
                    ui.end_row();
                }
            });
            ui.label(format!("Ettari bruciati: {:.0}", o.hectares));
            ui.separator();
        }
        ui.heading("Registro");
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            for e in &sim.log {
                ui.label(format!("{}  {}", clock(e.at_s), e.text));
            }
        });
    });

    // The operator's panel: a new game, by hand. Never automatic.
    egui::TopBottomPanel::bottom("operatore").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Operatore:");
            egui::ComboBox::from_id_source("caso").selected_text(k.case.clone()).show_ui(ui, |ui| {
                for c in k.territory.cases.clone() {
                    ui.selectable_value(&mut k.case, c.name.clone(), format!("{} (vicino a {})", c.name, c.near));
                }
            });
            ui.label("seme");
            ui.add(egui::DragValue::new(&mut k.seed).range(1..=999));
            if ui.button("Nuova partita").clicked() {
                new_game(k, &mut sim, &mut restarted);
            }
            if k.phase == Phase::Esegui && ui.button("Pausa").clicked() {
                k.enter(Phase::Pianifica);
            }
            if let Some(e) = &k.error {
                ui.colored_label(egui::Color32::RED, e);
            }
        });
    });

    focus.pointer = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
}
