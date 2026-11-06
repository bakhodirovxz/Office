//! Times the DX12 adapter probe of `apps/*/src/graphics.rs::probe_dx12` step by step.
use std::time::Instant;
use wgpu::{Backends, Instance, InstanceDescriptor};

fn main() {
    let t = Instant::now();
    let lap = |what: &str| eprintln!("{:>8.1} ms  {what}", t.elapsed().as_secs_f64() * 1000.0);
    let instance = Instance::new(InstanceDescriptor { backends: Backends::DX12, ..InstanceDescriptor::new_without_display_handle() });
    lap("Instance::new(DX12)");
    let adapters = pollster::block_on(instance.enumerate_adapters(Backends::DX12));
    lap(&format!("enumerate_adapters: {}", adapters.len()));
    for a in &adapters {
        let info = a.get_info();
        let ok = wgpu::Limits::default().check_limits(&a.limits());
        lap(&format!("  {} ({:?}) limits ok {ok}", info.name, info.device_type));
    }
    let only_hw = std::env::args().any(|a| a == "--drop-warp-first");
    let mut adapters = adapters;
    if only_hw {
        let warp = adapters.iter().position(|a| a.get_info().device_type == wgpu::DeviceType::Cpu).unwrap();
        drop(adapters.remove(warp));
        lap("drop WARP adapter");
    }
    drop(adapters);
    lap("drop adapters");
    drop(instance);
    lap("drop instance");
}
