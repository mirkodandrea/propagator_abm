//! The material of the closed roads: hazard tape, animated in the shader
//! (`shaders/tape.wgsl`). Laid on the road (a little above it, like the road's
//! own surface), not floating: no light, no fog, hidden by what stands in front.

use bevy::asset::uuid_handle;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

const TAPE_SHADER_HANDLE: Handle<Shader> = uuid_handle!("7a3e51c2-0b94-4d6f-8e17-c2a955b01d83");

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct TapeMaterial {
    /// x = 1: the soft variant, a thin dashed line for the part of a closed
    /// stretch that vehicles can still drive. y: saturation gain of the
    /// tactical view (`tactical.rs`), 0 = as drawn.
    #[uniform(0)]
    pub mode: Vec4,
}

impl TapeMaterial {
    /// Closed piece: full hazard tape.
    pub fn closed() -> Self {
        TapeMaterial { mode: Vec4::ZERO }
    }

    /// The rest of a closed stretch: thin, dashed, dim.
    pub fn open_part() -> Self {
        TapeMaterial { mode: Vec4::new(1.0, 0.0, 0.0, 0.0) }
    }
}

impl Material for TapeMaterial {
    // no depth prepass and no shadows (was `MaterialPlugin { prepass_enabled, shadows_enabled }` before Bevy 0.17)
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn vertex_shader() -> ShaderRef {
        TAPE_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        TAPE_SHADER_HANDLE.into()
    }
    fn depth_bias(&self) -> f32 {
        1.0
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

pub struct TapePlugin;

impl Plugin for TapePlugin {
    fn build(&self, app: &mut App) {
        crate::load_wesl!(app, TAPE_SHADER_HANDLE, "shaders/tape.wesl");
        app.add_plugins(MaterialPlugin::<TapeMaterial>::default());
    }
}
