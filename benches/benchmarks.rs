//! Performance benchmarks for Pulse

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use pulse::prelude::*;
use std::time::Duration;

fn benchmark_reverb(c: &mut Criterion) {
    let mut group = c.benchmark_group("reverb");
    group.measurement_time(Duration::from_secs(5));
    
    for block_size in [64, 128, 256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            block_size,
            |b, &size| {
                let mut reverb = Reverb::new(44100);
                reverb.set_room_size(0.8);
                reverb.set_damping(0.5);
                reverb.set_wet(0.3);
                
                let mut buffer = AudioBuffer::new(2, size);
                // Fill with test signal
                for ch in 0..2 {
                    if let Some(channel) = buffer.channel_mut(ch) {
                        for (i, sample) in channel.iter_mut().enumerate() {
                            *sample = (i as f32 * 0.1).sin();
                        }
                    }
                }
                
                let ctx = ProcessContext::default();
                
                b.iter(|| {
                    reverb.process(black_box(&mut buffer), black_box(&ctx));
                });
            },
        );
    }
    group.finish();
}

fn benchmark_delay(c: &mut Criterion) {
    let mut group = c.benchmark_group("delay");
    group.measurement_time(Duration::from_secs(5));
    
    for block_size in [64, 128, 256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            block_size,
            |b, &size| {
                let mut delay = Delay::new(44100);
                delay.set_time(0.25);
                delay.set_feedback(0.5);
                delay.set_mix(0.3);
                delay.set_mod_depth(2.0);
                delay.set_saturation(0.3);
                
                let mut buffer = AudioBuffer::new(2, size);
                for ch in 0..2 {
                    if let Some(channel) = buffer.channel_mut(ch) {
                        for (i, sample) in channel.iter_mut().enumerate() {
                            *sample = (i as f32 * 0.1).sin();
                        }
                    }
                }
                
                let ctx = ProcessContext::default();
                
                b.iter(|| {
                    delay.process(black_box(&mut buffer), black_box(&ctx));
                });
            },
        );
    }
    group.finish();
}

fn benchmark_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("compressor");
    group.measurement_time(Duration::from_secs(5));
    
    for block_size in [64, 128, 256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            block_size,
            |b, &size| {
                let mut compressor = Compressor::new(44100);
                compressor.set_threshold(-20.0);
                compressor.set_ratio(4.0);
                compressor.set_attack(10.0);
                compressor.set_release(100.0);
                
                let mut buffer = AudioBuffer::new(2, size);
                for ch in 0..2 {
                    if let Some(channel) = buffer.channel_mut(ch) {
                        for (i, sample) in channel.iter_mut().enumerate() {
                            *sample = (i as f32 * 0.1).sin() * 0.8;
                        }
                    }
                }
                
                let ctx = ProcessContext::default();
                
                b.iter(|| {
                    compressor.process(black_box(&mut buffer), black_box(&ctx));
                });
            },
        );
    }
    group.finish();
}

fn benchmark_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("eq");
    group.measurement_time(Duration::from_secs(5));
    
    // Standard IIR mode
    for block_size in [64, 128, 256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::new("iir", block_size),
            block_size,
            |b, &size| {
                let mut eq = ParametricEQ::new(44100);
                eq.set_band(0, 100.0, 3.0, 1.0);
                eq.set_band(1, 1000.0, -2.0, 1.5);
                eq.set_band(2, 5000.0, 2.0, 0.7);
                eq.set_linear_phase(false);
                
                let mut buffer = AudioBuffer::new(2, size);
                for ch in 0..2 {
                    if let Some(channel) = buffer.channel_mut(ch) {
                        for (i, sample) in channel.iter_mut().enumerate() {
                            *sample = (i as f32 * 0.1).sin();
                        }
                    }
                }
                
                let ctx = ProcessContext::default();
                
                b.iter(|| {
                    eq.process(black_box(&mut buffer), black_box(&ctx));
                });
            },
        );
    }
    
    // Linear phase (FFT) mode - only larger blocks
    for block_size in [256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::new("linear_phase", block_size),
            block_size,
            |b, &size| {
                let mut eq = ParametricEQ::new(44100);
                eq.set_band(0, 100.0, 3.0, 1.0);
                eq.set_band(1, 1000.0, -2.0, 1.5);
                eq.set_band(2, 5000.0, 2.0, 0.7);
                eq.set_linear_phase(true);
                
                let mut buffer = AudioBuffer::new(2, size);
                for ch in 0..2 {
                    if let Some(channel) = buffer.channel_mut(ch) {
                        for (i, sample) in channel.iter_mut().enumerate() {
                            *sample = (i as f32 * 0.1).sin();
                        }
                    }
                }
                
                let ctx = ProcessContext::default();
                
                b.iter(|| {
                    eq.process(black_box(&mut buffer), black_box(&ctx));
                });
            },
        );
    }
    group.finish();
}

fn benchmark_distortion(c: &mut Criterion) {
    let mut group = c.benchmark_group("distortion");
    group.measurement_time(Duration::from_secs(5));
    
    for block_size in [64, 128, 256, 512, 1024].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            block_size,
            |b, &size| {
                let mut distortion = Distortion::new(44100);
                distortion.set_drive(0.7);
                distortion.set_mix(0.5);
                
                let mut buffer = AudioBuffer::new(2, size);
                for ch in 0..2 {
                    if let Some(channel) = buffer.channel_mut(ch) {
                        for (i, sample) in channel.iter_mut().enumerate() {
                            *sample = (i as f32 * 0.1).sin();
                        }
                    }
                }
                
                let ctx = ProcessContext::default();
                
                b.iter(|| {
                    distortion.process(black_box(&mut buffer), black_box(&ctx));
                });
            },
        );
    }
    group.finish();
}

fn benchmark_automation(c: &mut Criterion) {
    use pulse::param::AutomationManager;
    use pulse::param::AutomationCurve;
    
    let mut group = c.benchmark_group("automation");
    group.measurement_time(Duration::from_secs(3));
    
    // Test process_block with various automation densities
    for point_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("process_block", point_count),
            point_count,
            |b, &count| {
                let mut manager = AutomationManager::new(44100.0);
                
                // Add automation points
                for i in 0..count {
                    let sample = (i * 44100 / count) as u64;
                    let value = (i as f32 / count as f32);
                    manager.add_point(0, sample, value, AutomationCurve::Linear);
                }
                
                manager.play();
                
                b.iter(|| {
                    manager.set_position(0);
                    let changes = manager.process_block(black_box(256));
                    black_box(changes);
                });
            },
        );
    }
    group.finish();
}

fn benchmark_process_256_samples(c: &mut Criterion) {
    // This benchmark specifically tests the spec requirement:
    // "Process 256 samples in < 1ms"
    
    let mut group = c.benchmark_group("spec_256_samples");
    group.measurement_time(Duration::from_secs(5));
    
    // All effects combined (worst case)
    group.bench_function("all_effects_chain", |b| {
        let mut reverb = Reverb::new(44100);
        let mut delay = Delay::new(44100);
        let mut compressor = Compressor::new(44100);
        let mut eq = ParametricEQ::new(44100);
        
        let mut buffer = AudioBuffer::new(2, 256);
        for ch in 0..2 {
            if let Some(channel) = buffer.channel_mut(ch) {
                for (i, sample) in channel.iter_mut().enumerate() {
                    *sample = (i as f32 * 0.1).sin();
                }
            }
        }
        
        let ctx = ProcessContext::default();
        
        b.iter(|| {
            reverb.process(black_box(&mut buffer), black_box(&ctx));
            delay.process(black_box(&mut buffer), black_box(&ctx));
            compressor.process(black_box(&mut buffer), black_box(&ctx));
            eq.process(black_box(&mut buffer), black_box(&ctx));
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_reverb,
    benchmark_delay,
    benchmark_compressor,
    benchmark_eq,
    benchmark_distortion,
    benchmark_automation,
    benchmark_process_256_samples,
);

criterion_main!(benches);
