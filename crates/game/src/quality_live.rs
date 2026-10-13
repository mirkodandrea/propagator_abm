//! The graphics level changed in the operator bar, put on screen (see `quality`).

use bevy::prelude::*;

use crate::quality::Graphics;

/// Put a changed [`Graphics`] on screen: the sample count, the shadows, the
/// post effects on the camera and the plants' LOD distance.
pub fn apply_live(
    gfx: Res<Graphics>,
    mut commands: Commands,
    mut msaa: ResMut<Msaa>,
    mut shadow_map: ResMut<bevy::pbr::DirectionalLightShadowMap>,
    mut suns: Query<(&mut DirectionalLight, &mut bevy::pbr::CascadeShadowConfig), With<crate::sky::Sun>>,
    cameras: Query<(
        Entity,
        Has<bevy::core_pipeline::bloom::BloomSettings>,
        Has<bevy::core_pipeline::dof::DepthOfFieldSettings>,
        &crate::camera::OrbitCamera,
    )>,
    veg: Option<ResMut<crate::vegetation::Vegetation>>,
) {
    use bevy::core_pipeline::{bloom::BloomSettings, dof::*, prepass::DepthPrepass};
    *msaa = gfx.msaa();
    if shadow_map.size != gfx.shadow_map {
        shadow_map.size = gfx.shadow_map;
    }
    for (mut light, mut cascades) in &mut suns {
        light.shadows_enabled = gfx.shadows;
        *cascades = bevy::pbr::CascadeShadowConfigBuilder {
            num_cascades: gfx.cascades,
            minimum_distance: 2.0,
            maximum_distance: gfx.shadow_distance,
            first_cascade_far_bound: 80.0,
            overlap_proportion: 0.3,
        }
        .build();
    }
    for (e, has_bloom, has_dof, orbit) in &cameras {
        let mut cam = commands.entity(e);
        match (gfx.bloom, has_bloom) {
            (true, false) => { cam.insert(BloomSettings { intensity: 0.20, ..BloomSettings::NATURAL }); }
            (false, true) => { cam.remove::<BloomSettings>(); }
            _ => {}
        }
        match (gfx.dof, has_dof) {
            (true, false) => {
                cam.insert((
                    DepthPrepass,
                    DepthOfFieldSettings {
                        mode: DepthOfFieldMode::Gaussian,
                        focal_distance: orbit.distance,
                        sensor_height: 0.01866,
                        aperture_f_stops: 0.012,
                        max_circle_of_confusion_diameter: 48.0,
                        max_depth: 12_000.0,
                    },
                ));
            }
            (false, true) => { cam.remove::<(DepthPrepass, DepthOfFieldSettings)>(); }
            _ => {}
        }
    }
    if let Some(mut veg) = veg {
        veg.set_lod_m(gfx.lod_m);
    }
}
