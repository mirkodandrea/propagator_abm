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
pub(crate) const FIGURE_SCALE: f32 = 3.0;

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
    car: Handle<Mesh>,
    /// Indexed by [`Status`].
    status: Vec<Handle<RetroMaterial>>,
    car_normal: Handle<RetroMaterial>,
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
    // A capsule reads as a person at any angle and costs almost nothing; the
    // silhouette is doing all the work at this distance anyway.
    let person: Handle<Mesh> =
        meshes.add(Capsule3d::new(0.32, 1.1).mesh().latitudes(4).longitudes(6));
    let car = meshes.add(Cuboid::new(1.8, 1.5, 4.3));

    // VR-training dev scenarios render everyone flat and unlit — the status
    // colours themselves stay meaningful (they are what the ABM testing
    // these scenarios exist for actually cares about), only the shading
    // model changes.
    let vr = sim.scenario.vr_palette().is_some();
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

    let car_normal = add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.90, 0.94),
        emissive: LinearRgba::rgb(0.20, 0.24, 0.32),
        perceptual_roughness: 0.4,
        unlit: vr,
        ..default()
    });
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
        commands.spawn((
            MaterialMeshBundle::<RetroMaterial> {
                mesh: person.clone(),
                material: status[Status::Evacuating as usize].clone(),
                transform: Transform::from_translation(frame::to_bevy(p.pos, ground))
                    .with_scale(Vec3::splat(FIGURE_SCALE)),
                visibility: Visibility::Hidden,
                ..default()
            },
            PersonView { id: p.id },
        ));
    }

    info!("people layer: {} figures", sim.agents.people.len());
    commands.insert_resource(PeopleAssets {
        car,
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
                mesh: assets.car.clone(),
                material: assets.car_normal.clone(),
                transform: Transform::from_translation(frame::to_bevy(t.pos, ground + 1.0))
                    .with_scale(Vec3::splat(FIGURE_SCALE * 0.8)),
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
        tf.translation = frame::to_bevy(p.pos, ground + 1.0 * FIGURE_SCALE * 0.5);
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
        tf.translation = frame::to_bevy(t.pos, ground + 1.2 * FIGURE_SCALE * 0.5);
        // World-frame bearing to a Bevy yaw: north is -Z, and the mesh's long
        // axis is +Z, so a heading of 0 (due east) is a quarter turn.
        tf.rotation = Quat::from_rotation_y(t.heading - std::f32::consts::FRAC_PI_2);

        let m = if t.state == TravelState::Cutoff {
            &assets.car_stuck
        } else {
            &assets.car_normal
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
    let mesh = meshes.add(Cylinder::new(12.0, 60.0));
    let mat = materials.add(retro::material(StandardMaterial {
        base_color: Color::srgba(0.30, 0.85, 0.95, 0.55),
        emissive: LinearRgba::rgb(0.15, 0.9, 1.1),
        alpha_mode: AlphaMode::Blend,
        ..default()
    }, sim.scenario.vr_palette().is_some()));
    for r in &sim.agents.refuges {
        let p = Pos {
            x: r.pos.x,
            y: r.pos.y,
        };
        let ground = sim.scenario.terrain.height_at(p);
        commands.spawn(MaterialMeshBundle::<RetroMaterial> {
            mesh: mesh.clone(),
            material: mat.clone(),
            transform: Transform::from_translation(frame::to_bevy(p, ground + 30.0)),
            ..default()
        });
    }
}
