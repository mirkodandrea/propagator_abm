//! Household and person entities.
//!
//! At ~750 households and ~1,600 people this is small enough to give every
//! household its own entity and mesh instance, which is what makes
//! click-to-inspect possible later. The data layout is kept flat (ids indexing
//! parallel arrays in the scenario) so scaling to tens of thousands means
//! swapping the renderer, not rewriting the model.

use bevy::prelude::*;
use scenario::population::Status;
use scenario::Pos;

use crate::sim::Sim;

#[derive(Component)]
pub struct HouseholdMarker {
    pub id: usize,
}

/// Floating indicator above each house, coloured by status. Same idea as
/// igad's beacons: the status has to be readable from commander altitude.
#[derive(Component)]
pub struct Beacon {
    pub household: usize,
}

pub fn status_color(status: Status) -> Color {
    match status {
        Status::Normal => Color::srgb(0.35, 0.85, 0.40),
        Status::Warned => Color::srgb(0.95, 0.85, 0.25),
        Status::Preparing => Color::srgb(0.98, 0.65, 0.15),
        Status::Evacuating => Color::srgb(0.30, 0.70, 0.98),
        Status::Evacuated => Color::srgb(0.25, 0.40, 0.85),
        Status::Defending => Color::srgb(0.85, 0.45, 0.90),
        Status::Trapped => Color::srgb(1.00, 0.25, 0.15),
        Status::Casualty => Color::srgb(0.15, 0.15, 0.15),
    }
}

pub fn spawn(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scn = &sim.scenario;
    let house_mesh = meshes.add(Cuboid::new(7.0, 5.0, 7.0));
    let beacon_mesh = meshes.add(Sphere::new(3.0).mesh().ico(2).unwrap());

    let house_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.72, 0.66),
        perceptual_roughness: 0.9,
        ..default()
    });

    // One material per status, shared across households, so status changes are
    // a material swap rather than an asset allocation.
    let status_mats: Vec<Handle<StandardMaterial>> = [
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
        let c = status_color(*s);
        materials.add(StandardMaterial {
            base_color: c,
            emissive: c.to_linear() * 2.0,
            ..default()
        })
    })
    .collect();

    for h in &scn.population.households {
        let p = Pos { x: h.pos[0], y: h.pos[1] };
        let ground = scn.terrain.height_at(p);

        commands.spawn((
            PbrBundle {
                mesh: house_mesh.clone(),
                material: house_mat.clone(),
                transform: Transform::from_xyz(p.x, ground + 2.5, -p.y),
                ..default()
            },
            HouseholdMarker { id: h.id },
        ));

        commands.spawn((
            PbrBundle {
                mesh: beacon_mesh.clone(),
                material: status_mats[0].clone(),
                transform: Transform::from_xyz(p.x, ground + 16.0, -p.y),
                ..default()
            },
            Beacon { household: h.id },
        ));
    }

    commands.insert_resource(StatusMaterials(status_mats));
    info!(
        "spawned {} households ({} people)",
        scn.population.households.len(),
        scn.population.people.len()
    );
}

#[derive(Resource)]
pub struct StatusMaterials(pub Vec<Handle<StandardMaterial>>);

/// Bob the beacons so they read as markers rather than scenery.
pub fn animate_beacons(time: Res<Time>, mut query: Query<(&Beacon, &mut Transform)>) {
    let t = time.elapsed_seconds();
    for (b, mut tf) in &mut query {
        let phase = b.household as f32 * 0.7;
        tf.translation.y += (t * 2.0 + phase).sin() * 0.02;
        tf.rotate_y(time.delta_seconds() * 0.8);
    }
}

/// Recolour beacons from the fire's structure-exposure model. Until the
/// behavioural model lands, this is what shows the fire actually reaching
/// people: exposure is proximity-based, because houses sit on non-burnable
/// cells and can never appear in the fire mask itself.
pub fn update_beacons(
    sim: Res<Sim>,
    mats: Res<StatusMaterials>,
    mut query: Query<(&Beacon, &mut Handle<StandardMaterial>)>,
) {
    if !sim.is_changed() {
        return;
    }
    let exposure = sim.fire.exposure();
    for (beacon, mut handle) in &mut query {
        let f = exposure.get(beacon.household);
        let status = if f.alight {
            Status::Casualty
        } else if f.radiant + f.ember > 0.35 {
            Status::Trapped
        } else if f.radiant + f.ember > 0.08 {
            Status::Preparing
        } else if f.ember > 0.0 {
            Status::Warned
        } else {
            Status::Normal
        };
        let idx = status as usize;
        if let Some(m) = mats.0.get(idx) {
            if *handle != *m {
                *handle = m.clone();
            }
        }
    }
}
