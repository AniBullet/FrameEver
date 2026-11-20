use std::sync::{Arc, Mutex};

const THREADS: u32 = 8;
const ROUNDS: u32 = 3000;
const LAYOUTS_PER_THREAD: u32 = 80;
const LAYOUTS_KEPT: usize = 32;

fn layout(device: &wgpu::Device, binding: u32) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    binding: u32,
    buffer: &wgpu::Buffer,
) {
    drop(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding,
            resource: buffer.as_entire_binding(),
        }],
    }));
}

#[test]
fn bind_group_layouts_made_on_one_thread_and_dropped_on_another_stay_usable() {
    let instance = crate::create_wgpu_instance_sync();
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        return;
    };
    let Ok((device, _queue)) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
    else {
        return;
    };
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let kept = Arc::new(Mutex::new(Vec::new()));

    std::thread::scope(|scope| {
        for thread in 0..THREADS {
            let (device, buffer, kept) = (&device, &buffer, kept.clone());
            scope.spawn(move || {
                for round in 0..ROUNDS {
                    let binding = thread * LAYOUTS_PER_THREAD + round % LAYOUTS_PER_THREAD;
                    let made = layout(device, binding);
                    bind(device, &made, binding, buffer);
                    let handed_over = {
                        let mut kept = kept.lock().unwrap();
                        kept.push((made, binding));
                        (kept.len() > LAYOUTS_KEPT).then(|| kept.swap_remove(0))
                    };
                    if let Some((layout, binding)) = handed_over {
                        bind(device, &layout, binding, buffer);
                    }
                }
            });
        }
    });
}
