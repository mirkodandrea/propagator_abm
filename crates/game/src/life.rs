//! Ambient life: the town before, and around, the fire.
//!
//! Without this the town is empty until somebody is told to leave, so the first
//! thing a student sees is a diorama of houses with nobody in it. These are the
//! neighbours going about their day: people walking the streets in twos and
//! ones, stopping to talk at the end of a lane, and a few cars on the local
//! roads. The people are not drawn here — [`crate::people`] owns every person
//! on the table, in ordinary life and affected alike, and asks this module only
//! for their beats ([`plan_walks`]); the cars are drawn here.
//!
//! **View only.** Nothing here is in the model and nothing here feeds back into
//! it. Each figure belongs to a real household and is a *pure function of the
//! wall clock and that household's status*, so there is nothing latched and a
//! restart needs no reset (finding 21). The status rule is the one thing that
//! ties it to the books (finding 43):
//!
//! | household | ambient figure (person or car) |
//! |---|---|
//! | `Normal` | walks its beat |
//! | `Warned`, `Preparing` | stops where it is and stands: something is wrong |
//! | anything else | gone — a person on the road is drawn at their real position |
//!
//! Beats are random walks along the real road network, straightish and without
//! backtracking, walked there and back with a pause at each end.

use abm::network::{NodeId, RoadNetwork, NO_NODE};
use std::collections::HashMap;

use bevy::prelude::*;
use scenario::population::Status;
use scenario::Pos;

use crate::frame;
use crate::models;
use crate::retro::{self, RetroMaterial};
use crate::sim::Sim;

/// Cars share [`crate::people::CAR_TOY`] with every other car in the scene.
const CAR_SCALE: f32 = crate::people::CAR_TOY;
/// Toy-world pace: fast enough to read as movement from table height.
const WALK_MS: f32 = 2.6;
const CAR_MS: f32 = 8.0;
/// Stand and chat at either end of a beat.
const PAUSE_S: f32 = 7.0;
const WALK_LEN_M: f32 = 260.0;
const CAR_LEN_M: f32 = 700.0;

/// A there-and-back beat along real streets with a pause at each end: the
/// whole of what a figure does in ordinary life. Shared by the ambient cars
/// and by [`crate::people`], which owns every person on the table.
pub(crate) struct Walk {
    /// Polyline in the world frame, with running distance at each vertex.
    path: Vec<Pos>,
    cum: Vec<f32>,
    speed: f32,
    /// Offset into the cycle so a street's worth of walkers are not in step.
    phase_s: f32,
    /// Sideways offset (m, world frame) so a pair walks abreast.
    side_m: f32,
}

/// Where a [`Walk`] has its figure at one instant.
pub(crate) struct Pose {
    pub pos: Pos,
    /// Unit direction of the segment (world frame), as walked outbound.
    ux: f32,
    uy: f32,
    pub moving: bool,
    pub outbound: bool,
}

impl Walk {
    fn new(path: Vec<Pos>, speed: f32, phase_s: f32, side_m: f32) -> Self {
        let cum = cumulative(&path);
        Walk { path, cum, speed, phase_s, side_m }
    }

    /// The pose at wall-clock `t`; `frozen` pins it to the start of the beat
    /// (a warned household's figure is at its own door, looking out).
    pub fn pose(&self, t: f32, frozen: bool) -> Pose {
        let (mut d, moving, outbound) = along(self, t);
        if frozen {
            d = 0.0;
        }
        let (p, ux, uy) = sample(self, d.clamp(0.0, *self.cum.last().unwrap()));
        // Walk abreast: offset to the figure's right, whichever way it faces.
        let sgn = if outbound { 1.0 } else { -1.0 };
        let pos = Pos { x: p.x + uy * self.side_m * sgn, y: p.y - ux * self.side_m * sgn };
        Pose { pos, ux, uy, moving: moving && !frozen, outbound }
    }
}

impl Pose {
    /// Yaw for a figure walking this pose (faces +Z at yaw 0).
    pub fn yaw(&self, car: bool) -> f32 {
        let (x, y) = if self.outbound { (self.ux, self.uy) } else { (-self.ux, -self.uy) };
        if car { y.atan2(x) + std::f32::consts::FRAC_PI_2 } else { (x).atan2(-y) }
    }
}

#[derive(Component)]
pub struct Ambient {
    household: usize,
    walk: Walk,
}

/// Deterministic hash to [0,1). No `rand` in the view: two runs of the kiosk on
/// one town should show the same people on the same streets.
fn unit(seed: u64) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// A straightish random walk of about `len_m` along the network from `start`.
fn beat(net: &RoadNetwork, start: NodeId, drivable: bool, len_m: f32, seed: u64) -> Vec<Pos> {
    let mut out = vec![net.pos(start)];
    let (mut at, mut prev, mut dist) = (start, NO_NODE, 0.0);
    let mut step = 0u64;
    while dist < len_m && step < 400 {
        step += 1;
        let here = net.pos(at);
        let heading = (prev != NO_NODE).then(|| {
            let p = net.pos(prev);
            let (dx, dy) = (here.x - p.x, here.y - p.y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-3);
            (dx / l, dy / l)
        });
        // Score each way on: straight on is best, a U-turn is never taken
        // unless the street is a dead end.
        let mut best = None;
        let mut best_score = f32::MIN;
        for (k, e) in net.neighbours(at).iter().enumerate() {
            if (drivable && !e.drivable) || e.to == prev {
                continue;
            }
            let q = net.pos(e.to);
            let (dx, dy) = (q.x - here.x, q.y - here.y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-3);
            let straight = heading.map_or(0.0, |h| (dx * h.0 + dy * h.1) / l);
            let score = straight + 0.5 * unit(seed ^ (step << 8) ^ k as u64);
            if score > best_score {
                best_score = score;
                best = Some((e.to, e.length_m));
            }
        }
        let Some((to, len)) = best else { break };
        prev = at;
        at = to;
        dist += len;
        out.push(net.pos(at));
    }
    out
}

fn cumulative(path: &[Pos]) -> Vec<f32> {
    let mut c = vec![0.0];
    for w in path.windows(2) {
        let l = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        c.push(c.last().unwrap() + l);
    }
    c
}

/// Marker: the ambient cars exist (the lab scenarios have none).
#[derive(Resource)]
pub struct LifeAssets;

/// The beats ordinary residents walk, keyed by person id: roughly one
/// household in five has someone out on the street, one in three of those
/// with company. Deterministic, so two runs on one town show the same people
/// on the same streets. Nobody else is drawn until they are affected.
pub(crate) fn plan_walks(sim: &Sim) -> HashMap<usize, Walk> {
    let mut out = HashMap::new();
    // The lab scenarios are flat-shaded test fixtures, not towns.
    if sim.scenario.vr_palette().is_some() {
        return out;
    }
    let net = &sim.agents.network;
    let homes = &sim.agents.households;
    if net.is_empty() || homes.is_empty() {
        return out;
    }
    let n = homes.len();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); n];
    for p in &sim.agents.people {
        if let Some(m) = members.get_mut(p.household) {
            m.push(p.id);
        }
    }
    for k in 0..(n / 5).clamp(12, 60) {
        // Spread across the town rather than clustered on the first ids.
        let h = (unit(k as u64 * 7919 + 1) * n as f32) as usize % n;
        let home = homes[h].home;
        let Some(start) = net.nearest(home, false) else { continue };
        let mut path = vec![home];
        path.extend(beat(net, start, false, WALK_LEN_M, h as u64 * 31 + k as u64));
        if path.len() < 3 {
            continue;
        }
        let pair = unit(k as u64 * 131 + 5) < 0.33;
        for (i, &person) in members[h].iter().take(1 + pair as usize).enumerate() {
            let phase = unit(k as u64 * 17 + 3) * 60.0 + i as f32 * 0.4;
            out.insert(person, Walk::new(path.clone(), WALK_MS, phase, if i == 1 { 1.6 } else { 0.0 }));
        }
    }
    info!("ambient life: {} walkers", out.len());
    out
}

pub fn setup(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    if sim.scenario.vr_palette().is_some() {
        return;
    }
    let net = &sim.agents.network;
    let homes = &sim.agents.households;
    if net.is_empty() || homes.is_empty() {
        return;
    }

    let cars: Vec<Handle<Mesh>> = ["car_hatch", "car_hatch", "car_sedan", "car_van", "car_ape"]
        .iter()
        .map(|n| meshes.add(models::kit_mesh(n)))
        .collect();
    let paint: Vec<Handle<RetroMaterial>> = [
        [0.93, 0.93, 0.95],
        [0.55, 0.78, 0.95],
        [0.98, 0.85, 0.40],
        [0.60, 0.85, 0.62],
        [0.30, 0.34, 0.40],
    ]
    .iter()
    .map(|c| {
        let col = Color::srgb(c[0], c[1], c[2]);
        materials.add(retro::material(
            StandardMaterial {
                base_color: col,
                emissive: col.to_linear() * 0.18,
                perceptual_roughness: 0.35,
                ..default()
            },
            false,
        ))
    })
    .collect();

    let n = homes.len();
    let drivers = (n / 25).clamp(3, 14);
    let pop = &sim.scenario.population;
    let mut spawned = 0;
    for k in 0..drivers {
        let h = (unit(k as u64 * 104_729 + 9) * n as f32) as usize % n;
        if pop.households.get(h).map_or(true, |x| x.vehicles == 0) {
            continue;
        }
        let Some(start) = net.nearest(homes[h].home, true) else { continue };
        let path = beat(net, start, true, CAR_LEN_M, h as u64 * 53 + k as u64);
        if path.len() < 4 {
            continue;
        }
        let ground = sim.scenario.terrain.height_at(path[0]);
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: cars[(h + k) % cars.len()].clone(),
                material: paint[(h + k * 3) % paint.len()].clone(),
                transform: Transform::from_translation(frame::to_bevy(path[0], ground + 0.05))
                    .with_scale(Vec3::splat(CAR_SCALE)),
                visibility: Visibility::Hidden,
                ..default()
            },
            Ambient {
                household: h,
                walk: Walk::new(path, CAR_MS, unit(k as u64 * 29 + 11) * 90.0, 0.0),
            },
        ));
        spawned += 1;
    }

    info!("ambient life: {} cars", spawned);
    commands.insert_resource(LifeAssets);
}

/// Where along `a` the figure is at `t`: there, a pause, back, a pause.
/// Returns distance along the path, whether it is moving, and whether it is
/// on the way out.
fn along(a: &Walk, t: f32) -> (f32, bool, bool) {
    let len = *a.cum.last().unwrap_or(&0.0);
    let leg = len / a.speed;
    let period = 2.0 * (leg + PAUSE_S);
    let u = (t + a.phase_s).rem_euclid(period);
    if u < leg {
        (u * a.speed, true, true)
    } else if u < leg + PAUSE_S {
        (len, false, true)
    } else if u < 2.0 * leg + PAUSE_S {
        (len - (u - leg - PAUSE_S) * a.speed, true, false)
    } else {
        (0.0, false, false)
    }
}

fn sample(a: &Walk, d: f32) -> (Pos, f32, f32) {
    let i = a.cum.partition_point(|&c| c <= d).clamp(1, a.cum.len() - 1);
    let (p, q) = (a.path[i - 1], a.path[i]);
    let seg = (a.cum[i] - a.cum[i - 1]).max(1e-3);
    let f = ((d - a.cum[i - 1]) / seg).clamp(0.0, 1.0);
    let (dx, dy) = (q.x - p.x, q.y - p.y);
    (Pos { x: p.x + dx * f, y: p.y + dy * f }, dx / seg, dy / seg)
}

/// Move, hide and freeze the ambient cars. Every frame: driven by the wall
/// clock and a plain lookup of the household's status.
pub fn update(
    time: Res<Time>,
    sim: Res<Sim>,
    mut query: Query<(&Ambient, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_seconds();
    for (a, mut tf, mut vis) in &mut query {
        let Some(h) = sim.agents.households.get(a.household) else { continue };
        let (show, frozen) = match h.status {
            Status::Normal => (true, false),
            Status::Warned | Status::Preparing => (true, true),
            _ => (false, false),
        };
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if !show {
            continue;
        }
        let pose = a.walk.pose(t, frozen);
        let ground = sim.scenario.terrain.height_at(pose.pos);
        tf.rotation = Quat::from_rotation_y(pose.yaw(true));
        tf.translation = frame::to_bevy(pose.pos, ground + 0.05);
    }
}
