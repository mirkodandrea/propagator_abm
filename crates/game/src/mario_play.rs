//! Mario's pastimes, just for fun (user, 2026-10-20): hidden stars, coins along
//! the roads, a greeting for every village and squad, a race from the port to
//! the rocca, and pretend flame-puffing with punches.
//!
//! Nothing here reaches `Game`: no order, no agent, no score, no fire cell. It
//! only reads where Mario stands (`Playground::pos`, `None` while he is not
//! out) and what he did this frame, so with Mario closed it does nothing.
//! Progress lives for the run of the program; only the race record is a file.

use super::Playground;
use crate::{frame, sim::Sim, AppState};
use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};
use bevy_egui::{egui, EguiContexts};
use scenario::Pos;
use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, PI};

/// Metres between two coins along a road, at least (fewer if there are many roads).
const COIN_EVERY_M: f32 = 90.;
const MAX_COINS: usize = 180;
const COIN_REACH: f32 = 8.;
const STAR_REACH: f32 = 14.;
const GREET_REACH: f32 = 90.;
const FLAME_REACH: f32 = 14.;
const GATE_REACH: f32 = 16.;
/// A jump this long in one frame is a relocation or a recovery, not running.
const TELEPORT_M: f32 = 100.;
/// Seconds a message stays on screen.
const TOAST_S: f32 = 4.5;
const FAKE_FIRE_NOTE: &str = "Fuoco finto: è solo uno scherzo, l’incendio vero non cambia.";

const BORGO_LINES: [&str; 10] = [
    "«Buongiorno, Mario! Qui è tutto tranquillo… per ora.»",
    "«Un idraulico? Magari ci serve per il lavello!»",
    "«Se vedi fumo, avvisaci subito: noi teniamo pronta la borsa.»",
    "«Mario! Ti offro un caffè, ma corri troppo veloce.»",
    "«Lassù si vede tutto il mare. Salutaci la rocca!»",
    "«Abbiamo fatto l’esercitazione: conosciamo la strada per l’area sicura.»",
    "«Bel cappello! Le ali sono di serie?»",
    "«I bambini volevano un autografo. Firma pure sul muro.»",
    "«Occhio al vento: oggi gira.»",
    "«Che bel baffo! Passa a trovarci quando c’è la sagra.»",
];
const TEAM_LINES: [&str; 5] = [
    "«Presente! Stiamo seguendo gli ordini del piano.»",
    "«Acqua a bordo, tre persone, pronti a partire.»",
    "«Bel salto, Mario! Ma ora lasciaci lavorare.»",
    "«Ricevuto: tieni le distanze dal fronte!»",
    "«Non è un gioco, Mario… ma un saluto ci sta.»",
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Star,
    Coin,
}

struct Star {
    at: Vec3,
    hint: String,
}

/// Meshes and materials, made once.
struct Look {
    coin: Handle<Mesh>,
    coin_mat: Handle<StandardMaterial>,
    star: Handle<Mesh>,
    star_mat: Handle<StandardMaterial>,
    beam: Handle<Mesh>,
    beam_mat: Handle<StandardMaterial>,
    gate_mat: Handle<StandardMaterial>,
    puff: Handle<Mesh>,
    puff_mat: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct Play {
    scenario: String,
    stars: Vec<Star>,
    coins: Vec<Vec3>,
    star_got: HashSet<usize>,
    coin_got: HashSet<usize>,
    greeted: HashSet<String>,
    greet_total: usize,
    entities: HashMap<(Kind, usize), Entity>,
    gates: Vec<Entity>,
    look: Option<Look>,
    /// Mario is out and the pastimes are drawn.
    out: bool,
    toast: Option<(String, f32)>,
    titles: Vec<&'static str>,
    flames: u32,
    flamed: HashSet<(usize, usize)>,
    flames_told: bool,
    punch_cd: f32,
    start: Vec3,
    finish: Vec3,
    /// Seconds on the clock while a race is on.
    race: Option<f32>,
    last_run: Option<f32>,
    best: Option<f32>,
    last_pos: Option<Vec3>,
}

impl Play {
    fn say(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), TOAST_S));
    }

    fn award(&mut self, title: &'static str, mode: &mut Playground) {
        if self.titles.contains(&title) {
            return;
        }
        self.titles.push(title);
        mode.victory = true;
        self.say(format!("Titolo: {title}!"));
    }
}

#[derive(Component)]
struct Spin(f32);

#[derive(Component)]
struct Bob {
    base: f32,
    phase: f32,
}

#[derive(Component)]
struct Puff(f32);

pub struct PlayPlugin;
impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Play>().add_systems(
            Update,
            (update, animate, panel).chain().run_if(in_state(AppState::Playing)),
        );
    }
}

fn island(sim: &Sim) -> Option<serde_json::Value> {
    let path = sim.data.join("scenarios").join(&sim.scn.id).join("island.json");
    serde_json::from_slice(&datafs::read(path).ok()?).ok()
}

fn pos_of(v: &serde_json::Value) -> Option<Pos> {
    Some(Pos { x: v[0].as_f64()? as f32, y: v[1].as_f64()? as f32 })
}

fn star_mesh(r_out: f32, r_in: f32, half: f32) -> Mesh {
    let ring: Vec<[f32; 2]> = (0..10)
        .map(|i| {
            let a = FRAC_PI_2 + i as f32 * PI / 5.;
            let r = if i % 2 == 0 { r_out } else { r_in };
            [r * a.cos(), r * a.sin()]
        })
        .collect();
    let (mut pos, mut nor, mut idx) = (Vec::<[f32; 3]>::new(), Vec::<[f32; 3]>::new(), Vec::<u32>::new());
    for (z, nz) in [(half, 1.), (-half, -1.)] {
        let base = pos.len() as u32;
        pos.push([0., 0., z]);
        nor.push([0., 0., nz]);
        for p in &ring {
            pos.push([p[0], p[1], z]);
            nor.push([0., 0., nz]);
        }
        for i in 0..10u32 {
            let (a, b) = (base + 1 + i, base + 1 + (i + 1) % 10);
            idx.extend(if nz > 0. { [base, a, b] } else { [base, b, a] });
        }
    }
    for i in 0..10 {
        let (p, q) = (ring[i], ring[(i + 1) % 10]);
        let n = Vec2::new(q[1] - p[1], p[0] - q[0]).normalize_or_zero();
        let base = pos.len() as u32;
        for (xy, z) in [(p, half), (q, half), (q, -half), (p, -half)] {
            pos.push([xy[0], xy[1], z]);
            nor.push([n.x, n.y, 0.]);
        }
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let uv = vec![[0., 0.]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

fn glow(r: f32, g: f32, b: f32, alpha: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(r, g, b, alpha),
        unlit: true,
        cull_mode: None,
        double_sided: true,
        alpha_mode: if alpha < 1. { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    }
}

fn record_path() -> Option<std::path::PathBuf> {
    // like the scores: a test run with its own scores file leaves the home alone
    if cfg!(target_arch = "wasm32") || std::env::var_os("KIOSK_SCORES").is_some() {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    Some(std::path::Path::new(&home).join(".rocca_ventosa").join("mario_record.txt"))
}

fn save_record(seconds: f32) {
    if let Some(path) = record_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, seconds.to_string());
    }
}

fn clock(seconds: f32) -> String {
    format!("{}:{:04.1}", (seconds / 60.) as u32, seconds % 60.)
}

fn compass(dx: f32, dy: f32) -> &'static str {
    // world y grows to the north
    let sector = ((dy.atan2(dx) / (PI / 4.)).round() as i32).rem_euclid(8);
    ["est", "nord-est", "nord", "nord-ovest", "ovest", "sud-ovest", "sud", "sud-est"][sector as usize]
}

/// Where the stars, coins and race gates stand: from the territory's data,
/// so every scenario gets its own.
fn build(play: &mut Play, sim: &Sim) {
    play.scenario = sim.scn.id.clone();
    play.stars.clear();
    play.coins.clear();
    play.star_got.clear();
    play.coin_got.clear();
    play.greeted.clear();
    play.titles.clear();
    play.flames = 0;
    play.flamed.clear();
    play.greet_total = sim.districts.len() + sim.teams.len();
    let ground = |p: Pos| sim.scn.terrain.height_at(p);
    let at = |p: Pos, up: f32| frame::to_bevy(p, ground(p).max(0.) + up);

    let island = island(sim);
    let landmark = |key: &str| island.as_ref().and_then(|i| pos_of(&i["landmarks"][key]["pos"]));
    if let Some(p) = landmark("rocca") {
        play.stars.push(Star { at: at(p, 45.), hint: "Dove l’isola è più alta e il vento è più forte: la rocca. Serve un gran salto.".into() });
        let wind = Pos { x: p.x + 300., y: p.y };
        play.stars.push(Star { at: at(wind, 120.), hint: "Più in alto di ogni albero, vicino alla rocca: solo con il Wing Cap.".into() });
    }
    if let Some(p) = landmark("torre") {
        play.stars.push(Star { at: at(p, 30.), hint: "Sul promontorio c’è una torre di pietra che guarda il mare.".into() });
    }
    if let Some(p) = island.as_ref().and_then(|i| pos_of(&i["port"]["shore"])) {
        play.stars.push(Star { at: at(p, 9.), hint: "Dove attraccano le barche, all’inizio del molo.".into() });
        play.start = at(p, 0.);
    }
    for d in &sim.districts {
        play.stars.push(Star { at: at(d.centre, 10.), hint: format!("Nel cuore di {}.", d.name) });
    }

    // coins: one every `step` metres along the roads, never in the sea
    let roads = &sim.scn.vectors.roads;
    let total: f32 = roads.iter().flat_map(|r| r.line.windows(2))
        .map(|s| Vec2::from_array(s[0]).distance(Vec2::from_array(s[1]))).sum();
    let step = COIN_EVERY_M.max(total / MAX_COINS as f32);
    for road in roads {
        let mut carry = step * 0.5;
        for s in road.line.windows(2) {
            let (a, b) = (Vec2::from_array(s[0]), Vec2::from_array(s[1]));
            let len = a.distance(b);
            let mut d = carry;
            while d < len {
                let q = a.lerp(b, d / len);
                let p = Pos { x: q.x, y: q.y };
                let v = at(p, 4.5);
                if !crate::sea::is_water(&sim.scn, p)
                    && play.coins.iter().all(|c| c.distance_squared(v) > 40. * 40.)
                    && play.coins.len() < MAX_COINS
                {
                    play.coins.push(v);
                }
                d += step;
            }
            carry = d - len;
        }
    }

    play.finish = super::rocca_start(sim).map_or(Vec3::ZERO, |(p, _)| p);
    play.best = record_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|t| t.trim().parse().ok());
}

fn look(play: &mut Play, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    play.look.get_or_insert_with(|| Look {
        coin: meshes.add(Cylinder::new(2.4, 0.6)),
        coin_mat: materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(1.2, 0.8, 0.0),
            ..glow(1., 0.78, 0.1, 1.)
        }),
        star: meshes.add(star_mesh(6., 2.6, 0.9)),
        star_mat: materials.add(glow(1., 0.9, 0.15, 1.)),
        beam: meshes.add(Cylinder::new(0.7, 1.)),
        beam_mat: materials.add(glow(1., 0.9, 0.3, 0.35)),
        gate_mat: materials.add(glow(0.2, 0.9, 1., 0.4)),
        puff: meshes.add(Sphere::new(1.6)),
        puff_mat: materials.add(glow(0.95, 0.95, 0.95, 0.7)),
    });
}

fn spawn_item(commands: &mut Commands, look: &Look, kind: Kind, at: Vec3, phase: f32) -> Entity {
    let (mesh, material, tilt, spin) = match kind {
        Kind::Coin => (look.coin.clone(), look.coin_mat.clone(), Quat::from_rotation_x(FRAC_PI_2), 3.),
        Kind::Star => (look.star.clone(), look.star_mat.clone(), Quat::IDENTITY, 1.6),
    };
    let beam = match kind {
        Kind::Star => Some(Vec3::new(1., 260., 1.)),
        Kind::Coin => None,
    };
    commands
        .spawn((SpatialBundle::from_transform(Transform::from_translation(at)), Spin(spin), Bob { base: at.y, phase }))
        .with_children(|p| {
            p.spawn(PbrBundle { mesh, material, transform: Transform::from_rotation(tilt), ..default() });
            if let Some(scale) = beam {
                p.spawn(PbrBundle {
                    mesh: look.beam.clone(),
                    material: look.beam_mat.clone(),
                    transform: Transform { translation: Vec3::Y * scale.y * 0.5, scale, ..default() },
                    ..default()
                });
            }
        })
        .id()
}

fn spawn_gate(commands: &mut Commands, look: &Look, at: Vec3) -> Entity {
    commands
        .spawn(PbrBundle {
            mesh: look.beam.clone(),
            material: look.gate_mat.clone(),
            transform: Transform { translation: at + Vec3::Y * 60., scale: Vec3::new(GATE_REACH * 0.5, 120., GATE_REACH * 0.5), ..default() },
            ..default()
        })
        .id()
}

fn clear_scene(commands: &mut Commands, play: &mut Play) {
    for (_, e) in play.entities.drain() {
        commands.entity(e).despawn_recursive();
    }
    for e in play.gates.drain(..) {
        commands.entity(e).despawn_recursive();
    }
    play.out = false;
    play.race = None;
    play.last_pos = None;
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    mut play: ResMut<Play>,
    mut mode: ResMut<Playground>,
    sim: Res<Sim>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_seconds().min(0.1);
    let Some(pos) = mode.pos.filter(|_| mode.active) else {
        if play.out {
            clear_scene(&mut commands, &mut play);
        }
        return;
    };
    if play.scenario != sim.scn.id {
        build(&mut play, &sim);
    }
    if !play.out {
        play.out = true;
        look(&mut play, &mut meshes, &mut materials);
        let look = play.look.as_ref().unwrap();
        let mut spawned = Vec::new();
        for (i, s) in play.stars.iter().enumerate().filter(|(i, _)| !play.star_got.contains(i)) {
            spawned.push(((Kind::Star, i), spawn_item(&mut commands, look, Kind::Star, s.at, i as f32)));
        }
        for (i, &c) in play.coins.iter().enumerate().filter(|(i, _)| !play.coin_got.contains(i)) {
            spawned.push(((Kind::Coin, i), spawn_item(&mut commands, look, Kind::Coin, c, i as f32)));
        }
        let gates = [play.start, play.finish].map(|g| spawn_gate(&mut commands, look, g));
        play.entities.extend(spawned);
        play.gates.extend(gates);
    }
    if let Some((_, left)) = play.toast.as_mut() {
        *left -= dt;
        if *left <= 0. {
            play.toast = None;
        }
    }
    play.punch_cd = (play.punch_cd - dt).max(0.);
    let body = pos + Vec3::Y * 4.;

    // stars and coins
    let got: Vec<usize> = play.stars.iter().enumerate()
        .filter(|(i, s)| !play.star_got.contains(i) && s.at.distance(body) < STAR_REACH)
        .map(|(i, _)| i).collect();
    for i in got {
        play.star_got.insert(i);
        if let Some(e) = play.entities.remove(&(Kind::Star, i)) {
            commands.entity(e).despawn_recursive();
        }
        mode.victory = true;
        let (n, total) = (play.star_got.len(), play.stars.len());
        play.say(format!("Stella! {n} su {total}"));
        if n == total {
            play.award("Esploratore di Roccavento", &mut mode);
        }
    }
    let got: Vec<usize> = play.coins.iter().enumerate()
        .filter(|(i, c)| !play.coin_got.contains(i) && c.distance(body) < COIN_REACH)
        .map(|(i, _)| i).collect();
    for i in got {
        play.coin_got.insert(i);
        if let Some(e) = play.entities.remove(&(Kind::Coin, i)) {
            commands.entity(e).despawn_recursive();
        }
        if play.coin_got.len() == play.coins.len() {
            play.award("Tesoriere dell’isola", &mut mode);
        }
    }

    // a greeting for every village and squad (F / L3)
    let engaged = std::mem::take(&mut mode.engaged);
    let unit = mode.greeted_unit.take();
    if engaged {
        let world = frame::to_world(pos);
        if let Some(id) = unit {
            let team = sim.teams.iter().position(|t| t.unit == id || t.crew_units.contains(&id));
            let (key, name, line) = match team {
                Some(t) => (format!("s:{t}"), sim.teams[t].name.clone(), TEAM_LINES[t % TEAM_LINES.len()]),
                None => ("s:?".into(), "Squadra".into(), TEAM_LINES[0]),
            };
            let first = play.greeted.insert(key);
            play.say(if first { format!("{name}: {line}") } else { format!("{name}: «Già salutati, Mario! Buon lavoro.»") });
        } else if let Some((i, d)) = sim.districts.iter().enumerate()
            .map(|(i, d)| (i, d, (d.centre.x - world.x).hypot(d.centre.y - world.y)))
            .filter(|t| t.2 < GREET_REACH)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|t| (t.0, t.1))
        {
            let first = play.greeted.insert(format!("b:{i}"));
            play.say(if first { format!("{}: {}", d.name, BORGO_LINES[i % BORGO_LINES.len()]) }
                else { format!("{}: «Ci siamo già salutati, ma un cenno fa sempre piacere.»", d.name) });
        }
        if play.greet_total > 0 && play.greeted.iter().filter(|k| *k != "s:?").count() >= play.greet_total {
            play.award("Cittadino onorario di Roccavento", &mut mode);
        }
    }

    // pretend flames: a punch puffs out the burning cells at hand, the fire stays as it is
    if std::mem::take(&mut mode.punched) && play.punch_cd == 0. {
        play.punch_cd = 0.35;
        let world = frame::to_world(pos);
        let near: Vec<Pos> = sim.fire.active_cells().iter()
            .filter(|c| (sim.scn.world.centre_of(**c).x - world.x).hypot(sim.scn.world.centre_of(**c).y - world.y) < FLAME_REACH)
            .filter(|c| play.flamed.insert((c.col, c.row)))
            .take(6)
            .map(|c| sim.scn.world.centre_of(*c))
            .collect();
        if !near.is_empty() {
            let look = play.look.as_ref().unwrap();
            for p in &near {
                commands.spawn((
                    PbrBundle {
                        mesh: look.puff.clone(),
                        material: look.puff_mat.clone(),
                        transform: Transform::from_translation(frame::to_bevy(*p, sim.scn.terrain.height_at(*p) + 3.)),
                        ..default()
                    },
                    Puff(0.),
                ));
            }
            play.flames += near.len() as u32;
            if !play.flames_told {
                play.flames_told = true;
                play.say(FAKE_FIRE_NOTE);
            }
        }
    }

    // the race, port to rocca
    if play.last_pos.is_some_and(|last| last.distance(pos) > TELEPORT_M) && play.race.take().is_some() {
        play.say("Corsa annullata: ti sei spostato.");
    }
    play.last_pos = Some(pos);
    let at_gate = |gate: Vec3| (gate.x - pos.x).hypot(gate.z - pos.z) < GATE_REACH && (gate.y - pos.y).abs() < 30.;
    if play.start != Vec3::ZERO && at_gate(play.start) {
        if play.race.is_none() {
            play.say("Via! Corri alla rocca.");
        }
        play.race = Some(0.);
    } else if let Some(t) = play.race {
        if play.finish != Vec3::ZERO && at_gate(play.finish) {
            play.race = None;
            play.last_run = Some(t);
            if play.best.map_or(true, |b| t < b) {
                play.best = Some(t);
                save_record(t);
                play.say(format!("Record! {}", clock(t)));
            } else {
                play.say(format!("Arrivato in {}", clock(t)));
            }
        } else {
            play.race = Some(t + dt);
        }
    }
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    mut spinning: Query<(&mut Transform, &Spin, &Bob), Without<Puff>>,
    mut puffs: Query<(Entity, &mut Transform, &mut Puff)>,
) {
    let t = time.elapsed_seconds();
    for (mut tf, spin, bob) in &mut spinning {
        tf.rotation = Quat::from_rotation_y(t * spin.0 + bob.phase);
        tf.translation.y = bob.base + (t * 2. + bob.phase).sin() * 0.8;
    }
    for (entity, mut tf, mut puff) in &mut puffs {
        puff.0 += time.delta_seconds();
        tf.translation.y += time.delta_seconds() * 4.;
        tf.scale = Vec3::splat(1. + puff.0 * 2.5);
        if puff.0 > 1.4 {
            commands.entity(entity).despawn();
        }
    }
}

fn panel(mut contexts: EguiContexts, play: Res<Play>, mode: Res<Playground>, sim: Res<Sim>) {
    let Some(pos) = mode.pos.filter(|_| mode.active) else { return };
    let ctx = contexts.ctx_mut();
    egui::Area::new(egui::Id::new("mario_play"))
        .anchor(egui::Align2::RIGHT_TOP, [-16., 16.])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(300.);
                ui.label(format!("Stelle {}/{}  ·  Monete {}/{}  ·  Saluti {}/{}",
                    play.star_got.len(), play.stars.len(), play.coin_got.len(), play.coins.len(),
                    play.greeted.iter().filter(|k| *k != "s:?").count(), play.greet_total));
                let next = play.stars.iter().enumerate()
                    .filter(|(i, _)| !play.star_got.contains(i))
                    .min_by(|a, b| a.1.at.distance(pos).total_cmp(&b.1.at.distance(pos)));
                if let Some((_, star)) = next {
                    let (dx, dy) = (star.at.x - pos.x, -(star.at.z - pos.z));
                    ui.small(format!("Indizio: {} (circa {:.0} m a {})", star.hint, dx.hypot(dy), compass(dx, dy)));
                }
                let world = frame::to_world(pos);
                if let Some(d) = sim.districts.iter().find(|d| (d.centre.x - world.x).hypot(d.centre.y - world.y) < GREET_REACH) {
                    ui.small(format!("F / L3: saluta {}", d.name));
                }
                match (play.race, play.best) {
                    (Some(t), _) => { ui.label(format!("Corsa: {}", clock(t))); }
                    (None, Some(b)) => { ui.small(format!("Corsa porto → rocca · record {}", clock(b))); }
                    (None, None) => { ui.small("Corsa: dal porto (colonna azzurra) alla rocca."); }
                }
                if let Some(t) = play.last_run {
                    ui.small(format!("Ultima corsa: {}", clock(t)));
                }
                if play.flames > 0 {
                    ui.small(format!("Fiammelle spente: {} (solo scena: il fuoco vero non cambia)", play.flames));
                }
                for title in &play.titles {
                    ui.colored_label(egui::Color32::GOLD, format!("★ {title}"));
                }
            });
        });
    if let Some((text, _)) = &play.toast {
        egui::Area::new(egui::Id::new("mario_play_toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0., -40.])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(text).size(20.));
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_and_compass() {
        assert_eq!(clock(75.25), "1:15.2");
        assert_eq!(compass(10., 0.), "est");
        assert_eq!(compass(0., 10.), "nord");
        assert_eq!(compass(-10., -10.), "sud-ovest");
    }

    #[test]
    fn star_mesh_is_closed_and_indexed() {
        let m = star_mesh(6., 2.6, 0.9);
        let n = m.count_vertices();
        assert!(m.indices().unwrap().iter().all(|i| i < n));
    }

    #[test]
    fn items_are_built_for_the_real_territory_and_stay_apart() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let sim = Sim::new(&data, "medium", 1).unwrap();
        let mut play = Play::default();
        build(&mut play, &sim);
        assert!(play.stars.len() >= 4, "stars: {}", play.stars.len());
        assert!(!play.coins.is_empty() && play.coins.len() <= MAX_COINS, "coins: {}", play.coins.len());
        assert!(play.start != Vec3::ZERO && play.finish != Vec3::ZERO);
        assert!(play.greet_total > sim.districts.len());
    }
}
