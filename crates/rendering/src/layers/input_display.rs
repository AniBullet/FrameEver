use std::{collections::HashMap, sync::Arc};

use bytemuck::{Pod, Zeroable};
use frameever_project::{
    InputDisplayStyle,
    input_display::{
        DisplayRect, InputRecord, Sprite, display_rect, input_record_generation,
        take_at,
    },
};
use wgpu::util::DeviceExt;

use crate::{
    DecodedSegmentFrames, ProjectUniforms, RenderVideoConstants,
    input_display::{LoadedPreset, atlas_scale_for, preset_for_controller, resolve_preset},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[derive(Clone, PartialEq, Eq)]
struct AtlasKey {
    preset: String,
    style: InputDisplayStyle,
    scale: u32,
    generation: u64,
}

struct Atlas {
    key: AtlasKey,
    loaded: Option<LoadedPreset>,
    bind_group: Option<wgpu::BindGroup>,
}

#[derive(Clone, PartialEq)]
struct Drawn {
    sprites: Vec<Sprite>,
    rect: DisplayRect,
    output_size: (u32, u32),
    atlas: AtlasKey,
}

pub struct InputDisplayLayer {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    opacity: wgpu::Buffer,
    atlas: Option<Atlas>,
    vertices: Option<wgpu::Buffer>,
    vertex_count: u32,
    drawn: Option<Drawn>,
    records: HashMap<usize, Arc<InputRecord>>,
    records_generation: u64,
    has_content: bool,
}

impl InputDisplayLayer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader =
            device.create_shader_module(wgpu::include_wgsl!("../shaders/input_display.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Input display pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Input display sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            opacity: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Input display opacity"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            atlas: None,
            vertices: None,
            vertex_count: 0,
            drawn: None,
            records: HashMap::new(),
            records_generation: input_record_generation(),
            has_content: false,
        }
    }

    pub fn has_content(&self) -> bool {
        self.has_content
    }

    fn ensure_atlas(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, key: AtlasKey) {
        if self.atlas.as_ref().is_some_and(|atlas| atlas.key == key) {
            return;
        }
        let loaded = resolve_preset(&key.preset, key.style, key.scale);
        let bind_group = loaded.as_ref().map(|loaded| {
            let size = wgpu::Extent3d {
                width: loaded.atlas.width(),
                height: loaded.atlas.height(),
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Input display atlas"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                loaded.atlas.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width * 4),
                    rows_per_image: Some(size.height),
                },
                size,
            );
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Input display bind group"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.opacity.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        });
        self.atlas = Some(Atlas {
            key,
            loaded,
            bind_group,
        });
    }

    pub fn prepare(
        &mut self,
        constants: &RenderVideoConstants,
        uniforms: &ProjectUniforms,
        segment_frames: &DecodedSegmentFrames,
    ) {
        self.has_content = false;
        let Some(block) = uniforms.project.shown_input_display() else {
            return;
        };
        let output_size = uniforms.output_size;
        if output_size.0 == 0 || output_size.1 == 0 {
            return;
        }
        let output_time = uniforms.frame_number as f64 / uniforms.frame_rate.max(1) as f64;
        let take = take_at(
            &uniforms.project,
            output_time,
            f64::from(segment_frames.segment_time),
        );
        let records_generation = input_record_generation();
        if records_generation != self.records_generation {
            self.records.clear();
            self.records_generation = records_generation;
        }
        let record = self
            .records
            .entry(take)
            .or_insert_with(|| Arc::new(InputRecord::for_take(&constants.recording_meta, take)));
        let state = record.state_at(f64::from(segment_frames.recording_time) * 1000.0);
        let preset = preset_for_controller(&block.preset, state.controller);

        let generation = crate::input_presets::generation();
        let mut key = AtlasKey {
            preset: preset.to_string(),
            style: block.style,
            scale: self
                .atlas
                .as_ref()
                .filter(|atlas| {
                    atlas.key.preset == preset
                        && atlas.key.style == block.style
                        && atlas.key.generation == generation
                })
                .map_or(1, |atlas| atlas.key.scale),
            generation,
        };
        self.ensure_atlas(&constants.device, &constants.queue, key.clone());
        let Some(loaded) = self.atlas.as_ref().and_then(|atlas| atlas.loaded.as_ref()) else {
            return;
        };
        let rect = display_rect(
            block,
            (loaded.preset.width, loaded.preset.height),
            loaded.frame_share,
            output_size,
        );
        let wanted = atlas_scale_for(loaded.preset.width, rect.width);
        if loaded.scalable && wanted != loaded.atlas_scale {
            key.scale = wanted;
            self.ensure_atlas(&constants.device, &constants.queue, key.clone());
        }
        let Some(Atlas {
            loaded: Some(loaded),
            bind_group: Some(_),
            ..
        }) = self.atlas.as_ref()
        else {
            return;
        };

        constants.queue.write_buffer(
            &self.opacity,
            0,
            bytemuck::cast_slice(&[block.opacity_fraction() as f32, 0.0, 0.0, 0.0]),
        );

        let drawn = Drawn {
            sprites: loaded.preset.sprites(&state),
            rect,
            output_size,
            atlas: key,
        };
        if self.drawn.as_ref() != Some(&drawn) || self.vertices.is_none() {
            let vertices = quads(&drawn, loaded);
            self.vertex_count = vertices.len() as u32;
            self.vertices = Some(constants.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Input display vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
            self.drawn = Some(drawn);
        }
        self.has_content = self.vertex_count > 0;
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        let (Some(vertices), Some(bind_group)) = (
            &self.vertices,
            self.atlas
                .as_ref()
                .and_then(|atlas| atlas.bind_group.as_ref()),
        ) else {
            return;
        };
        if !self.has_content {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}

fn quads(drawn: &Drawn, loaded: &LoadedPreset) -> Vec<Vertex> {
    let (output_width, output_height) = (
        f64::from(drawn.output_size.0),
        f64::from(drawn.output_size.1),
    );
    let per_unit = drawn.rect.width / f64::from(loaded.preset.width.max(1));
    let atlas_scale = f64::from(loaded.atlas_scale.max(1));
    let (atlas_width, atlas_height) = (
        f64::from(loaded.atlas.width().max(1)),
        f64::from(loaded.atlas.height().max(1)),
    );
    let mut vertices = Vec::with_capacity(drawn.sprites.len() * 6);
    for sprite in &drawn.sprites {
        let left = drawn.rect.x + f64::from(sprite.x) * per_unit;
        let top = drawn.rect.y + f64::from(sprite.y) * per_unit;
        let right = left + f64::from(sprite.source.width) * per_unit;
        let bottom = top + f64::from(sprite.source.height) * per_unit;
        let u0 = f64::from(sprite.source.u) * atlas_scale / atlas_width;
        let v0 = f64::from(sprite.source.v) * atlas_scale / atlas_height;
        let u1 = f64::from(sprite.source.u + sprite.source.width) * atlas_scale / atlas_width;
        let v1 = f64::from(sprite.source.v + sprite.source.height) * atlas_scale / atlas_height;
        let corner = |x: f64, y: f64, u: f64, v: f64| Vertex {
            position: [
                (x / output_width * 2.0 - 1.0) as f32,
                (1.0 - y / output_height * 2.0) as f32,
            ],
            uv: [u as f32, v as f32],
        };
        vertices.extend([
            corner(left, top, u0, v0),
            corner(right, top, u1, v0),
            corner(left, bottom, u0, v1),
            corner(left, bottom, u0, v1),
            corner(right, top, u1, v0),
            corner(right, bottom, u1, v1),
        ]);
    }
    vertices
}
