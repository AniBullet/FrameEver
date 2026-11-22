use super::{CompositeVideoFramePipeline, CompositeVideoFrameUniforms};

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: CompositeVideoFramePipeline,
}

fn gpu() -> Option<Gpu> {
    let instance = crate::create_wgpu_instance_sync();
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available, skipping downscale filter tests");
        return None;
    };
    eprintln!("Downscale filter adapter: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("create downscale filter test device");
    let pipeline = CompositeVideoFramePipeline::new(&device);
    Some(Gpu {
        device,
        queue,
        pipeline,
    })
}

struct Source {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Source {
    fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Self {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Self {
            width,
            height,
            rgba,
        }
    }
}

struct Draw {
    output: [u32; 2],
    crop: [f32; 4],
    target: [f32; 4],
    preserve_alpha: bool,
    motion_blur: [f32; 2],
    detail: bool,
}

struct Prepared {
    bind_group: wgpu::BindGroup,
    output: wgpu::Texture,
}

fn prepare(gpu: &Gpu, source: &Source, draw: &Draw) -> Prepared {
    let Gpu {
        device,
        queue,
        pipeline,
    } = gpu;
    let frame = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: source.width,
            height: source.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &frame,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &source.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(source.width * 4),
            rows_per_image: Some(source.height),
        },
        frame.size(),
    );
    let [width, height] = draw.output;
    let target_size = [
        draw.target[2] - draw.target[0],
        draw.target[3] - draw.target[1],
    ];
    let uniforms = CompositeVideoFrameUniforms {
        output_size: [width as f32, height as f32],
        frame_size: [source.width as f32, source.height as f32],
        crop_bounds: draw.crop,
        target_bounds: draw.target,
        target_size,
        rounding_px: 0.0,
        corner_radii: [0.0; 4],
        preserve_source_alpha: if draw.preserve_alpha { 1.0 } else { 0.0 },
        motion_blur_vector: draw.motion_blur,
        motion_blur_params: if draw.motion_blur == [0.0; 2] {
            [0.0; 4]
        } else {
            [1.0, 1.0, 0.0, 0.0]
        },
        enhance_params: [0.0, if draw.detail { 1.0 } else { 0.0 }, 0.0, 0.0],
        ..Default::default()
    };
    let frame_view = frame.create_view(&Default::default());
    let uniforms_buffer = uniforms.to_buffer(device);
    let bind_group = pipeline.bind_group(device, &uniforms_buffer, &frame_view);
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    Prepared { bind_group, output }
}

fn draw_into(gpu: &Gpu, prepared: &Prepared, encoder: &mut wgpu::CommandEncoder) {
    let view = prepared.output.create_view(&Default::default());
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&gpu.pipeline.render_pipeline);
    pass.set_bind_group(0, &prepared.bind_group, &[]);
    pass.draw(0..3, 0..1);
}

fn read_back(gpu: &Gpu, output: &wgpu::Texture, mut encoder: wgpu::CommandEncoder) -> Vec<u8> {
    let (width, height) = (output.width(), output.height());
    let padded_row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (padded_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        output.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("wait for downscale readback");
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .expect("map downscale readback");
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * padded_row) as usize;
        pixels.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
    }
    pixels
}

fn render(gpu: &Gpu, source: &Source, draw: &Draw) -> Vec<u8> {
    let prepared = prepare(gpu, source, draw);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    draw_into(gpu, &prepared, &mut encoder);
    read_back(gpu, &prepared.output, encoder)
}

fn full_frame_draw(source: &Source, output: [u32; 2]) -> Draw {
    Draw {
        output,
        crop: [0.0, 0.0, source.width as f32, source.height as f32],
        target: [0.0, 0.0, output[0] as f32, output[1] as f32],
        preserve_alpha: false,
        motion_blur: [0.0; 2],
        detail: false,
    }
}

fn luma_spread(pixels: &[u8], width: u32, inset: u32) -> (f64, f64) {
    let height = pixels.len() as u32 / 4 / width;
    let mut values = Vec::new();
    for y in inset..height - inset {
        for x in inset..width - inset {
            values.push(pixels[((y * width + x) * 4 + 1) as usize] as f64);
        }
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
    (mean, variance.sqrt())
}

#[test]
fn one_to_one_draw_returns_the_source() {
    let Some(gpu) = gpu() else { return };
    let source = Source::from_fn(96, 64, |x, y| {
        let v = ((x * 37 + y * 91 + (x * y) % 13 * 17) % 256) as u8;
        [v, v.wrapping_mul(3), 255 - v, 255]
    });
    let pixels = render(&gpu, &source, &full_frame_draw(&source, [96, 64]));
    for y in 3..61 {
        for x in 3..93 {
            let at = ((y * 96 + x) * 4) as usize;
            assert_eq!(pixels[at..at + 4], source.rgba[at..at + 4], "pixel {x},{y}");
        }
    }
}

#[test]
fn shrunk_one_pixel_stripes_average_instead_of_beating() {
    let Some(gpu) = gpu() else { return };
    let source = Source::from_fn(192, 96, |x, _| {
        let v = if x % 2 == 0 { 0 } else { 255 };
        [v, v, v, 255]
    });
    let output = [128, 64];
    for preserve_alpha in [false, true] {
        let draw = Draw {
            preserve_alpha,
            ..full_frame_draw(&source, output)
        };
        let pixels = render(&gpu, &source, &draw);
        let (mean, spread) = luma_spread(&pixels, output[0], 3);
        assert!(
            (mean - 127.5).abs() < 4.0,
            "preserve_alpha {preserve_alpha}: mean {mean:.1}"
        );
        assert!(
            spread < 45.0,
            "preserve_alpha {preserve_alpha}: spread {spread:.1}"
        );
    }
}

#[test]
fn shrunk_cutout_edges_keep_their_colour() {
    let Some(gpu) = gpu() else { return };
    let source = Source::from_fn(192, 96, |x, y| {
        if (40..150).contains(&x) && (20..76).contains(&y) {
            [255, 255, 255, 255]
        } else {
            [0, 0, 0, 0]
        }
    });
    let draw = Draw {
        preserve_alpha: true,
        ..full_frame_draw(&source, [64, 32])
    };
    let pixels = render(&gpu, &source, &draw);
    let mut edge_pixels = 0;
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        let (x, y) = (index as u32 % 64, index as u32 / 64);
        let (cx, cy) = (3 * x + 1, 3 * y + 1);
        if !((40..150).contains(&cx) && (20..76).contains(&cy)) {
            continue;
        }
        let alpha = pixel[3] as i32;
        for channel in &pixel[..3] {
            assert!(
                (*channel as i32 - alpha).abs() <= 2,
                "pixel {x},{y} is {pixel:?}"
            );
        }
        if alpha < 255 {
            edge_pixels += 1;
        }
    }
    assert!(
        edge_pixels > 0,
        "the shrunk edge should be partly transparent"
    );
}

#[test]
fn heavily_shrunk_stripes_stay_flat() {
    let Some(gpu) = gpu() else { return };
    let source = Source::from_fn(330, 99, |x, _| {
        let v = if x % 2 == 0 { 0 } else { 255 };
        [v, v, v, 255]
    });
    let output = [100, 30];
    let pixels = render(&gpu, &source, &full_frame_draw(&source, output));
    let (mean, spread) = luma_spread(&pixels, output[0], 4);
    assert!((mean - 127.5).abs() < 4.0, "mean {mean:.1}");
    assert!(spread < 25.0, "spread {spread:.1}");
}

#[test]
fn motion_blur_keeps_the_shrink_filter() {
    let Some(gpu) = gpu() else { return };
    let source = Source::from_fn(330, 99, |_, y| {
        let v = if y % 2 == 0 { 0 } else { 255 };
        [v, v, v, 255]
    });
    let output = [100, 30];
    let still = render(&gpu, &source, &full_frame_draw(&source, output));
    let moving = render(
        &gpu,
        &source,
        &Draw {
            motion_blur: [0.02, 0.0],
            ..full_frame_draw(&source, output)
        },
    );
    let (_, still_spread) = luma_spread(&still, output[0], 4);
    let (mean, moving_spread) = luma_spread(&moving, output[0], 4);
    assert!((mean - 127.5).abs() < 4.0, "mean {mean:.1}");
    assert!(
        moving_spread < 25.0,
        "still {still_spread:.1}, moving {moving_spread:.1}"
    );
}

#[test]
#[ignore]
fn benchmark_shrunk_layer_draw() {
    let Some(gpu) = gpu() else { return };
    let cases = [
        ("1080p shown 1:1", [1920, 1080], [1920, 1080], [0.0; 2]),
        (
            "1440p in 1080p (1.33x)",
            [2560, 1440],
            [1920, 1080],
            [0.0; 2],
        ),
        ("4K in 1080p (2x)", [3840, 2160], [1920, 1080], [0.0; 2]),
        ("4K in 720p (3x)", [3840, 2160], [1280, 720], [0.0; 2]),
        (
            "4K in 720p (3x), motion blur",
            [3840, 2160],
            [1280, 720],
            [0.02, 0.0],
        ),
    ];
    const DRAWS_PER_SUBMIT: u32 = 50;
    const ROUNDS: usize = 9;
    for (name, [source_width, source_height], output, motion_blur) in cases {
        let source = Source::from_fn(source_width, source_height, |x, y| {
            let v = ((x * 37 + y * 91) % 256) as u8;
            [v, v.wrapping_mul(3), 255 - v, 255]
        });
        let prepared = prepare(
            &gpu,
            &source,
            &Draw {
                motion_blur,
                ..full_frame_draw(&source, output)
            },
        );
        let mut per_draw_ms = (0..=ROUNDS)
            .map(|_| {
                let started = std::time::Instant::now();
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                for _ in 0..DRAWS_PER_SUBMIT {
                    draw_into(&gpu, &prepared, &mut encoder);
                }
                read_back(&gpu, &prepared.output, encoder);
                started.elapsed().as_secs_f64() * 1000.0 / DRAWS_PER_SUBMIT as f64
            })
            .skip(1)
            .collect::<Vec<_>>();
        per_draw_ms.sort_by(f64::total_cmp);
        eprintln!(
            "{name}: median {:.3} ms per draw, min {:.3}",
            per_draw_ms[ROUNDS / 2],
            per_draw_ms[0]
        );
    }
}

fn text_like(width: u32, height: u32) -> Source {
    Source::from_fn(width, height, |x, y| {
        let ink = (x / 3 + y / 5) % 4 == 0 || (x * 7 + y * 3) % 11 < 2;
        let v = if ink { 30 } else { 230 };
        [v, v, v, 255]
    })
}

#[test]
fn detail_filter_leaves_large_shrinks_and_their_blur_alone() {
    let Some(gpu) = gpu() else { return };
    let source = text_like(330, 99);
    let output = [100, 30];
    for blur in [0.0, 0.002, 0.005, 0.01, 0.02] {
        let draw = |detail| Draw {
            motion_blur: [blur, 0.0],
            detail,
            ..full_frame_draw(&source, output)
        };
        assert_eq!(
            render(&gpu, &source, &draw(false)),
            render(&gpu, &source, &draw(true)),
            "blur {blur}"
        );
    }
}

#[test]
fn detail_filter_fades_in_from_full_size() {
    let Some(gpu) = gpu() else { return };
    let source = text_like(420, 210);
    let change = |ratio: f32| {
        let output = [(420.0 / ratio).ceil() as u32, (210.0 / ratio).ceil() as u32];
        let draw = |detail| Draw {
            target: [0.0, 0.0, 420.0 / ratio, 210.0 / ratio],
            detail,
            ..full_frame_draw(&source, output)
        };
        let off = render(&gpu, &source, &draw(false));
        let on = render(&gpu, &source, &draw(true));
        let total: u64 = off
            .iter()
            .zip(&on)
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum();
        total as f64 / off.len() as f64
    };
    assert_eq!(change(1.0), 0.0);
    let steps = [1.005, 1.02, 1.04, 1.06].map(change);
    assert!(
        steps.windows(2).all(|pair| pair[0] <= pair[1]),
        "change should grow towards full strength: {steps:?}"
    );
    assert!(
        steps[0] * 4.0 < steps[3],
        "a near-1:1 shrink should barely change: {steps:?}"
    );
}

#[test]
fn detail_filter_keeps_sharpness_when_motion_stops() {
    let Some(gpu) = gpu() else { return };
    let source = text_like(500, 250);
    let output = [200, 100];
    let worst = |detail| {
        let draw = |blur| Draw {
            motion_blur: [blur, 0.0],
            detail,
            ..full_frame_draw(&source, output)
        };
        let still = render(&gpu, &source, &draw(0.0));
        let settling = render(&gpu, &source, &draw(0.0001));
        still
            .iter()
            .zip(&settling)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0)
    };
    let (plain, detail) = (worst(false), worst(true));
    assert!(
        detail <= plain + 1,
        "largest difference {detail} levels with the filter, {plain} without"
    );
}
