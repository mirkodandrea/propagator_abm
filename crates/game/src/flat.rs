//! The material of the signs on the map (hexes, routes, the forecast): a
//! flat, bright colour that takes no light and no fog, drawn over the canopy.
//! The terrain's material (`materials::TerrainMaterial`) is for the scenery; a sign drawn with it
//! fades with the haze and the tone mapper, and sinks under the trees.

use bevy::asset::uuid_handle;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

const FLAT_SHADER_HANDLE: Handle<Shader> = uuid_handle!("5c1e7a0b-93d4-4f28-b6e1-0d2a7f35c9e4");

/// How far toward the camera a sign is pulled: this fraction of its
/// distance, between the two bounds (metres). Above the tallest trees on
/// the near ground, small enough that a ridge in front still hides it.
const PULL: Vec4 = Vec4::new(0.04, 40.0, 220.0, 0.0);

#[derive(Clone, Copy, ShaderType)]
pub struct FlatUniform {
    color: Vec4,
    pull: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct FlatMaterial {
    #[uniform(0)]
    pub uniform: FlatUniform,
    pub alpha: AlphaMode,
    /// Order among the transparent signs: higher is drawn later, on top.
    pub order: f32,
}

impl FlatMaterial {
    /// `c` as sRGB, opaque.
    pub fn new(c: [f32; 3]) -> Self {
        Self::with_alpha(c, 1.0)
    }

    /// `c` as sRGB, with alpha `a` (blended under 1).
    pub fn with_alpha(c: [f32; 3], a: f32) -> Self {
        let l = Color::srgb(c[0], c[1], c[2]).to_linear();
        FlatMaterial {
            uniform: FlatUniform { color: Vec4::new(l.red, l.green, l.blue, a), pull: PULL },
            alpha: if a < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
            order: 0.0,
        }
    }

    pub fn saturation(&self) -> f32 {
        self.uniform.pull.w
    }

    /// Saturation gain of the tactical view (`tactical.rs`): 0 = as given.
    pub fn set_saturation(&mut self, gain: f32) {
        self.uniform.pull.w = gain;
    }

    /// Pulled a little less (or more) toward the camera than the default:
    /// an under-stroke sits behind its line.
    pub fn pulled(mut self, extra_m: f32) -> Self {
        self.uniform.pull.y += extra_m;
        self.uniform.pull.z += extra_m;
        self.order = extra_m;
        self
    }
}

impl Material for FlatMaterial {
    // no depth prepass and no shadows (was `MaterialPlugin { prepass_enabled, shadows_enabled }` before Bevy 0.17)
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn vertex_shader() -> ShaderRef {
        FLAT_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        FLAT_SHADER_HANDLE.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        self.alpha
    }
    fn depth_bias(&self) -> f32 {
        self.order
    }
    fn specialize(
        _: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _: &bevy::mesh::MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub struct FlatPlugin;

impl Plugin for FlatPlugin {
    fn build(&self, app: &mut App) {
        crate::load_wesl!(app, FLAT_SHADER_HANDLE, "shaders/flat.wesl");
        // no prepass: it would write the depth of the sign where it is, not
        // where it is drawn
        app.add_plugins(MaterialPlugin::<FlatMaterial>::default());
    }
}
