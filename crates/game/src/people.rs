//! Drawing the people and their vehicles.
//!
//! Two layers, because they answer different questions:
//!
//! - **people**, one entity per simulated person, so the claim that this is an
//!   individual-agent model is visible rather than asserted. They appear when
//!   they are outside — walking out, or caught in the open — and are hidden
//!   while indoors or once they are away.
//! - **vehicles**, one per travelling household that took a car. The car is
//!   the household, so its members ride hidden inside it.
//!
//! Everything here is drawn well above life size. From incident-command
//! altitude a 1.7 m figure is a fraction of a pixel, and a fraction of a pixel
//! that flickers in and out of existence is worse than nothing: the whole
//! point of the layer is to see the evacuation move. The exaggeration is a
//! deliberate map-symbol choice, in the same family as the status beacons, and
//! [`FIGURE_SCALE`] is the single knob.

use abm::{Mode, TravelState};
use bevy::prelude::*;
use scenario::population::Status;
use scenario::Pos;

use crate::frame;
use crate::retro;
use crate::retro::RetroMaterial;
use crate::sim::Sim;

/// How far above life size people and cars are drawn. See the module note.
/// `pub(crate)` so [`crate::inspect`] can pick at the same height these are
/// actually drawn at, rather than duplicating the constant and drifting.
pub(crate) const FIGURE_SCALE: f32 = 3.5;

/// One scale for every car in the scene — parked in a lot (`town_kit`), driving
/// the network, or ambient (`life`) — so a queue of evacuees is made of the
/// same cars the car park is. Houses are drawn at `TOY_SCALE` (1.5, footprint)
/// and 1.7 tall; a kit hatchback is 4 m, so ×2 is an 8 m toy car against a
/// 15 m toy house: bigger than life against the house, because it has to read
/// at play altitude, but not bigger than the house.
pub(crate) const CAR_TOY: f32 = 2.0;

pub(crate) fn figure_scale(vr: bool) -> f32 {
    // A person is an operational map symbol in a lab scenario. At the fitted
    // stage view the realistic scale is sub-pixel, so exaggerate it enough to
    // preserve the status colour without approaching the size of a building.
    FIGURE_SCALE * if vr { 2.4 } else { 1.0 }
}

#[derive(Component)]
pub struct PersonView {
    pub id: usize,
}

#[derive(Component)]
pub struct VehicleView {
    pub traveller: usize,
}

#[derive(Resource)]
pub struct PeopleAssets {
    /// Body types, picked per household so a queue is a street's worth of
    /// different cars (and the odd Ape) rather than one car repeated.
    cars: Vec<Handle<Mesh>>,
    /// Indexed by [`Status`].
    status: Vec<Handle<RetroMaterial>>,
    car_normal: Vec<Handle<RetroMaterial>>,
    car_stuck: Handle<RetroMaterial>,
    /// Vehicles already given an entity, so new departures can be picked up
    /// without rescanning.
    spawned_vehicles: usize,
}

pub fn setup(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    // Figures by age: children, adults, the elderly with a stick. The clothes
    // are white in the bake so the status tint still carries the meaning.
    let figure: Vec<Handle<Mesh>> = ["person_man", "person_woman", "person_child", "person_elder"]
        .iter()
        .map(|n| meshes.add(crate::models::kit_mesh(n)))
        .collect();
    // Weighted: mostly small hatchbacks and saloons, as on any Italian road.
    let cars: Vec<Handle<Mesh>> = ["car_hatch", "car_hatch", "car_hatch", "car_sedan", "car_sedan", "car_suv", "car_van", "car_ape", "car_camper"]
        .iter()
        .map(|n| meshes.add(crate::models::kit_mesh(n)))
        .collect();

    // VR-training dev scenarios render everyone flat and unlit — the status
    // colours themselves stay meaningful (they are what the ABM testing
    // these scenarios exist for actually cares about), only the shading
    // model changes.
    let vr = sim.scenario.vr_palette().is_some();
    let scale = figure_scale(vr);
    let mut add = |base: StandardMaterial| materials.add(retro::material(base, vr));
    let status: Vec<Handle<RetroMaterial>> = [
        Status::Normal,
        Status::Warned,
        Status::Preparing,
        Status::Evacuating,
        Status::Evacuated,
        Status::Defending,
        Status::Trapped,
        Status::Casualty,
    ]
    .iter()
    .map(|s| {
        let c = crate::agents::status_color(*s);
        add(StandardMaterial {
            base_color: c,
            // A little self-illumination, or a figure in the smoke shadow
            // vanishes exactly when the player most needs to see it.
            emissive: c.to_linear() * 0.9,
            perceptual_roughness: 0.8,
            unlit: vr,
            ..default()
        })
    })
    .collect();

    // Toy cars come in pastel paint so a queue reads as individual cars.
    let car_normal: Vec<Handle<RetroMaterial>> = [
        [0.93, 0.93, 0.95],
        [0.55, 0.78, 0.95],
        [0.98, 0.85, 0.40],
        [0.60, 0.85, 0.62],
        [0.95, 0.65, 0.65],
        [0.75, 0.70, 0.92],
        [0.85, 0.20, 0.18],
        [0.30, 0.34, 0.40],
    ]
    .iter()
    .map(|c| {
        let col = Color::srgb(c[0], c[1], c[2]);
        add(StandardMaterial {
            base_color: col,
            emissive: col.to_linear() * 0.18,
            perceptual_roughness: 0.35,
            unlit: vr,
            ..default()
        })
    })
    .collect();
    let car_stuck = add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.35, 0.20),
        emissive: LinearRgba::rgb(0.9, 0.22, 0.05),
        perceptual_roughness: 0.5,
        unlit: vr,
        ..default()
    });

    // People all exist from the start; visibility is what changes.
    for p in &sim.agents.people {
        let ground = sim.scenario.terrain.height_at(p.pos);
        let age = sim.scenario.population.people.get(p.id).map_or(40, |q| q.age);
        let mesh = match age {
            a if a < 14 => &figure[2],
            a if a >= 70 => &figure[3],
            _ => &figure[p.id % 2],
        };
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: mesh.clone(),
                material: status[Status::Evacuating as usize].clone(),
                transform: Transform::from_translation(frame::to_bevy(p.pos, ground))
                    .with_scale(Vec3::splat(scale)),
                visibility: Visibility::Hidden,
                ..default()
            },
            PersonView { id: p.id },
        ));
    }

    info!("people layer: {} figures", sim.agents.people.len());
    commands.insert_resource(PeopleAssets {
        cars,
        status,
        car_normal,
        car_stuck,
        spawned_vehicles: 0,
    });
}

/// Drop the vehicles when the sim restarts.
///
/// `VehicleView` holds an *index* into `Abm::travellers`, which is append-only
/// within a run — which is what makes [`spawn_vehicles`] a cheap tail scan, and
/// what makes these entities meaningless the moment the list is rebuilt. The
/// people are not touched: they are keyed by person id, and the population is
/// the same population.
pub fn reset(
    mut commands: Commands,
    mut restarted: EventReader<crate::sim::SimRestarted>,
    mut assets: ResMut<PeopleAssets>,
    vehicles: Query<Entity, With<VehicleView>>,
) {
    if restarted.is_empty() {
        return;
    }
    restarted.clear();
    for e in &vehicles {
        commands.entity(e).despawn();
    }
    assets.spawned_vehicles = 0;
}

/// Give an entity to every household that has taken to the road since the last
/// frame. Travellers are append-only, so this is a tail scan.
pub fn spawn_vehicles(mut commands: Commands, sim: Res<Sim>, mut assets: ResMut<PeopleAssets>) {
    let total = sim.agents.travellers.len();
    if total == assets.spawned_vehicles {
        return;
    }
    for i in assets.spawned_vehicles..total {
        let t = &sim.agents.travellers[i];
        if t.mode != Mode::Car {
            continue;
        }
        let ground = sim.scenario.terrain.height_at(t.pos);
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: assets.cars[(t.household.wrapping_mul(2654435761) >> 7) % assets.cars.len()].clone(),
                material: assets.car_normal[i % assets.car_normal.len()].clone(),
                transform: Transform::from_translation(frame::to_bevy(t.pos, ground + 0.05))
                    .with_scale(Vec3::splat(CAR_TOY * if sim.scenario.vr_palette().is_some() { 2.4 } else { 1.0 })),
                ..default()
            },
            VehicleView { traveller: i },
        ));
    }
    assets.spawned_vehicles = total;
}

pub fn update_people(
    sim: Res<Sim>,
    assets: Res<PeopleAssets>,
    mut query: Query<(
        &PersonView,
        &mut Transform,
        &mut Visibility,
        &mut Handle<RetroMaterial>,
    )>,
) {
    if !sim.is_changed() {
        return;
    }
    for (view, mut tf, mut vis, mut mat) in &mut query {
        let Some(p) = sim.agents.people.get(view.id) else {
            continue;
        };
        // Indoors, or gone: not drawn. A person riding in a car is drawn as
        // the car.
        let in_vehicle = p
            .traveller
            .and_then(|t| sim.agents.travellers.get(t))
            .map(|t| t.mode == Mode::Car && t.state != TravelState::Cutoff)
            .unwrap_or(false);
        let outside = matches!(p.status, Status::Evacuating | Status::Trapped) && !in_vehicle;
        let want = if outside {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if !outside {
            continue;
        }

        let ground = sim.scenario.terrain.height_at(p.pos);
        let next = frame::to_bevy(p.pos, ground + 0.05);
        let movement = next - tf.translation;
        if movement.x * movement.x + movement.z * movement.z > 0.0001 {
            tf.rotation = Quat::from_rotation_y(movement.x.atan2(movement.z));
        }
        tf.translation = next;
        let m = &assets.status[p.status as usize];
        if *mat != *m {
            *mat = m.clone();
        }
    }
}

pub fn update_vehicles(
    sim: Res<Sim>,
    assets: Res<PeopleAssets>,
    mut query: Query<(
        &VehicleView,
        &mut Transform,
        &mut Visibility,
        &mut Handle<RetroMaterial>,
    )>,
) {
    if !sim.is_changed() {
        return;
    }
    for (view, mut tf, mut vis, mut mat) in &mut query {
        let Some(t) = sim.agents.travellers.get(view.traveller) else {
            continue;
        };
        // A car whose occupants abandoned it on foot stays where it was left,
        // which is exactly what a blocked road looks like from the air.
        let driving = matches!(t.state, TravelState::Approaching | TravelState::OnNetwork)
            && t.mode == Mode::Car;
        let abandoned = t.mode == Mode::Foot || t.state == TravelState::Cutoff;
        let want = if driving || abandoned {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if !driving {
            continue;
        }

        let ground = sim.scenario.terrain.height_at(t.pos);
        tf.translation = frame::to_bevy(t.pos, ground + 0.05);
        // World-frame bearing to a Bevy yaw: north is -Z, and the mesh's long
        // axis is +Z, so a heading of 0 (due east) is a quarter turn.
        tf.rotation = Quat::from_rotation_y(t.heading + std::f32::consts::FRAC_PI_2);

        let m = if t.state == TravelState::Cutoff {
            &assets.car_stuck
        } else {
            &assets.car_normal[view.traveller % assets.car_normal.len()]
        };
        if *mat != *m {
            *mat = m.clone();
        }
    }
}

/// Mark the refuges people are heading for. Static: the set is chosen once at
/// load, from the fuel raster (see `abm::refuge`).
pub fn mark_refuges(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RetroMaterial>>,
) {
    // A refuge is a place: a piazza disc, a blue sign on a post. Drawn at 1.4x
    // so the sign clears the canopy and the town.
    let mesh = meshes.add(crate::toy::refuge_sign());
    let mat = materials.add(retro::material(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::rgb(0.05, 0.08, 0.12),
        perceptual_roughness: 0.6,
        ..default()
    }, false));
    for r in &sim.agents.refuges {
        let p = Pos {
            x: r.pos.x,
            y: r.pos.y,
        };
        let ground = sim.scenario.terrain.height_at(p);
        commands.spawn(MaterialMeshBundle::<RetroMaterial> {
            mesh: mesh.clone(),
            material: mat.clone(),
            transform: Transform::from_translation(frame::to_bevy(p, ground + 0.3))
                .with_scale(Vec3::splat(1.4)),
            ..default()
        });
    }
}

/// Walking bob: figures hop and squash a little, so a column of evacuees reads
/// as people walking rather than as sliding pins. Every frame, cheap.
pub fn bob_people(
    time: Res<Time>,
    mut query: Query<(&PersonView, &mut Transform, &Visibility)>,
) {
    let t = time.elapsed_seconds();
    for (view, mut tf, vis) in &mut query {
        if *vis == Visibility::Hidden {
            continue;
        }
        let ph = t * 9.0 + view.id as f32 * 1.7;
        let k = FIGURE_SCALE;
        tf.scale = Vec3::new(k * (1.0 - 0.05 * ph.sin()), k * (1.0 + 0.10 * ph.sin()), k * (1.0 - 0.05 * ph.sin()));
    }
}
