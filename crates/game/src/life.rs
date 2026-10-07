//! Ambient life: the town before, and around, the fire.
//!
//! Without this the town is empty until somebody is told to leave, so the first
//! thing a student sees is a diorama of houses with nobody in it. These are the
//! neighbours going about their day: people walking the streets in twos and
//! ones, stopping to talk at the end of a lane, and a few cars on the local
//! roads.
//!
//! **View only.** Nothing here is in the model and nothing here feeds back into
//! it. Each figure belongs to a real household and is a *pure function of the
//! wall clock and that household's status*, so there is nothing latched and a
//! restart needs no reset (finding 21). The status rule is the one thing that
//! ties it to the books (finding 43):
//!
//! | household | ambient figure |
//! |---|---|
//! | `Normal` | walks its beat |
//! | `Warned`, `Preparing` | stops where it is and stands: something is wrong |
//! | anything else | gone — the real evacuees in [`crate::people`] take over |
//!
//! Beats are random walks along the real road network, straightish and without
//! backtracking, walked there and back with a pause at each end.

use abm::network::{NodeId, RoadNetwork, NO_NODE};
use bevy::prelude::*;
use scenario::population::Status;
use scenario::Pos;

use crate::frame;
use crate::models;
use crate::retro::{self, RetroMaterial};
use crate::sim::Sim;

/// A little under the evacuees' [`crate::people::FIGURE_SCALE`], so the two
/// never read as the same thing (muted clothes, never the status colours,
/// finish the job), and cars share [`crate::people::CAR_TOY`] with every other
/// car in the scene.
const WALKER_SCALE: f32 = 3.0;
const CAR_SCALE: f32 = crate::people::CAR_TOY;
/// Toy-world pace: fast enough to read as movement from table height.
const WALK_MS: f32 = 2.6;
const CAR_MS: f32 = 8.0;
/// Stand and chat at either end of a beat.
const PAUSE_S: f32 = 7.0;
const WALK_LEN_M: f32 = 260.0;
const CAR_LEN_M: f32 = 700.0;

#[derive(Component)]
pub struct Ambient {
    household: usize,
    /// Polyline in the world frame, with running distance at each vertex.
    path: Vec<Pos>,
    cum: Vec<f32>,
    speed: f32,
    /// Offset into the cycle so a street's worth of walkers are not in step.
    phase_s: f32,
    /// Sideways offset (m, world frame) so a pair walks abreast.
    side_m: f32,
    car: bool,
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

#[derive(Resource)]
pub struct LifeAssets {
    walker: Vec<Handle<Mesh>>,
    clothes: Vec<Handle<RetroMaterial>>,
}

pub fn setup(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    // The lab scenarios are flat-shaded test fixtures, not towns.
    if sim.scenario.vr_palette().is_some() {
        return;
    }
    let net = &sim.agents.network;
    let homes = &sim.agents.households;
    if net.is_empty() || homes.is_empty() {
        return;
    }

    let figure: Vec<Handle<Mesh>> = ["person_man", "person_woman", "person_child", "person_elder"]
        .iter()
        .map(|n| meshes.add(models::kit_mesh(n)))
        .collect();
    // Muted dress, deliberately off the status palette (green/amber/red).
    let clothes: Vec<Handle<RetroMaterial>> = [
        [0.62, 0.52, 0.42],
        [0.45, 0.52, 0.64],
        [0.70, 0.62, 0.48],
        [0.55, 0.40, 0.42],
        [0.78, 0.76, 0.70],
        [0.36, 0.42, 0.40],
    ]
    .iter()
    .map(|c| {
        let col = Color::srgb(c[0], c[1], c[2]);
        materials.add(retro::material(
            StandardMaterial {
                base_color: col,
                emissive: col.to_linear() * 0.25,
                perceptual_roughness: 0.8,
                ..default()
            },
            false,
        ))
    })
    .collect();
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
    let walkers = (n / 5).clamp(12, 60);
    let drivers = (n / 25).clamp(3, 14);
    let pop = &sim.scenario.population;
    let mut spawned = (0, 0);

    for k in 0..walkers {
        // Spread across the town rather than clustered on the first ids.
        let h = (unit(k as u64 * 7919 + 1) * n as f32) as usize % n;
        let home = homes[h].home;
        let Some(start) = net.nearest(home, false) else { continue };
        let mut path = vec![home];
        path.extend(beat(net, start, false, WALK_LEN_M, h as u64 * 31 + k as u64));
        if path.len() < 3 {
            continue;
        }
        // One in three goes out with company, walking abreast.
        let pair = unit(k as u64 * 131 + 5) < 0.33;
        for member in 0..(1 + pair as usize) {
            let age = pop.people.get(h).map_or(40, |q| q.age);
            let mesh = match (member, age) {
                (_, a) if a >= 70 => &figure[3],
                (1, _) => &figure[2],
                _ => &figure[(h + member) % 2],
            };
            let cum = cumulative(&path);
            let ground = sim.scenario.terrain.height_at(home);
            commands.spawn((
                MaterialMeshBundle::<RetroMaterial> {
                    mesh: mesh.clone(),
                    material: clothes[(h * 5 + k + member * 2) % clothes.len()].clone(),
                    transform: Transform::from_translation(frame::to_bevy(home, ground))
                        .with_scale(Vec3::splat(WALKER_SCALE)),
                    visibility: Visibility::Hidden,
                    ..default()
                },
                Ambient {
                    household: h,
                    path: path.clone(),
                    cum,
                    speed: WALK_MS,
                    phase_s: unit(k as u64 * 17 + 3) * 60.0 + member as f32 * 0.4,
                    side_m: if member == 1 { 1.6 } else { 0.0 },
                    car: false,
                },
            ));
            spawned.0 += 1;
        }
    }

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
        let cum = cumulative(&path);
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
                path,
                cum,
                speed: CAR_MS,
                phase_s: unit(k as u64 * 29 + 11) * 90.0,
                side_m: 0.0,
                car: true,
            },
        ));
        spawned.1 += 1;
    }

    info!("ambient life: {} walkers, {} cars", spawned.0, spawned.1);
    commands.insert_resource(LifeAssets { walker: figure, clothes });
}

/// Where along `a` the figure is at `t`: there, a pause, back, a pause.
/// Returns distance along the path and whether it is moving.
fn along(a: &Ambient, t: f32) -> (f32, bool, bool) {
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

fn sample(a: &Ambient, d: f32) -> (Pos, f32, f32) {
    let i = a.cum.partition_point(|&c| c <= d).clamp(1, a.cum.len() - 1);
    let (p, q) = (a.path[i - 1], a.path[i]);
    let seg = (a.cum[i] - a.cum[i - 1]).max(1e-3);
    let f = ((d - a.cum[i - 1]) / seg).clamp(0.0, 1.0);
    let (dx, dy) = (q.x - p.x, q.y - p.y);
    (Pos { x: p.x + dx * f, y: p.y + dy * f }, dx / seg, dy / seg)
}

/// Move, hide and freeze the ambient figures. Every frame: it is driven by the
/// wall clock, and the household status it reads is a plain lookup.
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

        let (mut d, moving, outbound) = along(a, t);
        if frozen {
            // Stateless, so no memory of where the figure was: a warned
            // household's figure is simply at its own door (a car, at the
            // kerb it starts from), looking out. It reads as stepping outside.
            d = 0.0;
        }
        let (p, ux, uy) = sample(a, d.clamp(0.0, *a.cum.last().unwrap()));
        // Walk abreast: offset to the figure's right, whichever way it faces.
        let sgn = if outbound { 1.0 } else { -1.0 };
        let p = Pos { x: p.x + uy * a.side_m * sgn, y: p.y - ux * a.side_m * sgn };
        let ground = sim.scenario.terrain.height_at(p);
        let lift = if a.car { 0.05 } else { 0.0 };
        let next = frame::to_bevy(p, ground + lift);

        if a.car {
            let heading = if outbound { uy.atan2(ux) } else { (-uy).atan2(-ux) };
            tf.rotation = Quat::from_rotation_y(heading + std::f32::consts::FRAC_PI_2);
        } else if moving && !frozen {
            let m = next - tf.translation;
            if m.x * m.x + m.z * m.z > 1e-5 {
                tf.rotation = Quat::from_rotation_y(m.x.atan2(m.z));
            }
        }
        tf.translation = next;

        // Walking bob; standing figures breathe instead.
        if !a.car {
            let ph = t * if moving && !frozen { 8.0 } else { 1.6 } + a.household as f32 * 1.7;
            let amp = if moving && !frozen { 0.10 } else { 0.02 };
            let k = WALKER_SCALE;
            tf.scale = Vec3::new(k * (1.0 - amp * 0.5 * ph.sin()), k * (1.0 + amp * ph.sin()), k * (1.0 - amp * 0.5 * ph.sin()));
        }
    }
}
