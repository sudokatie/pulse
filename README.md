# Pulse

Audio plugin framework in Rust. Host VST3/AU/CLAP plugins or build your own effects. 370 tests. Zero allocations in the audio path.

## Why This Exists?

Because audio software shouldn't require a PhD to understand. The existing plugin frameworks are either C++ nightmares (VST3 SDK, I'm looking at you) or bloated wrappers that add latency. Pulse is a from-scratch Rust implementation that treats real-time safety as a requirement, not an afterthought.

Also, writing a reverb from scratch is a rite of passage. Don't deny yourself that joy.

## Features

- **Built-in effects**: Algorithmic reverb (Freeverb), stereo delay with feedback, compressor/limiter with sidechain, parametric EQ (linear phase mode), distortion with waveshaping, biquad filters
- **Plugin hosting**: Load VST3, AU (macOS), and CLAP plugins
- **Plugin development**: Trait-based API for building your own effects
- **MIDI input**: Cross-platform MIDI via midir, with CC mapping and note triggers
- **Parameter automation**: Smooth parameter transitions, automation lanes, curve editing
- **Preset system**: Save/load plugin configurations with search and categories
- **Real-time safe**: Lock-free audio path, no allocations during processing
- **CLI**: Process audio files, list plugins, apply effects from the command line

## Quick Start

```bash
# Build
cargo build --release

# Run tests
cargo test

# Apply reverb to an audio file
pulse effect reverb --input input.wav --output output.wav --room-size 0.8 --wet 0.3

# Apply delay
pulse effect delay --input input.wav --output output.wav --time 375ms --feedback 0.4

# List available plugins
pulse plugins list

# Process with a hosted VST3 plugin
pulse effect plugin --input input.wav --plugin /path/to/plugin.vst3 --preset "Warm Hall"
```

## Built-in Effects

### Reverb (Freeverb-style)
Eight comb filters + four allpass filters. Classic algorithmic reverb that sounds surprisingly decent.
- Room size, damping, wet/dry mix
- Stereo width control

### Delay
Stereo delay with tempo-synced timing, feedback loop, and optional saturation.
- Tempo sync (ms or BPM-based)
- Feedback with saturation
- Ping-pong mode
- Low-pass filter in feedback path

### Compressor/Limiter
Peak and RMS detection with sidechain input, lookahead, and flexible knee.
- Peak/RMS detection modes
- Adjustable attack, release, threshold, ratio, knee
- Sidechain input
- Makeup gain
- Lookahead buffer for true peak limiting

### Parametric EQ
FFT-based linear phase mode or standard IIR biquad cascade.
- Unlimited bands
- Filter types: low/high shelf, peak, low/high pass, bandpass, notch
- Linear phase mode via overlap-add FFT
- Per-band gain, frequency, Q

### Distortion
Waveshaping with multiple curves and pre/post gain.
- Curves: soft clip, hard clip, tube, foldback, bitcrush
- Pre-gain and post-gain
- Tone control (low-pass filter)

### Biquad Filter
Building block for everything else. Also useful standalone.
- Low-pass, high-pass, bandpass, notch, allpass, peak, low/high shelf
- Direct Form II Transposed
- Coefficient calculation from frequency, Q, gain

## Plugin API

Build your own effects:

```rust
use pulse::prelude::*;

struct MyEffect {
    sample_rate: f32,
    gain: f32,
}

impl Plugin for MyEffect {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("My Effect", "my-effect")
            .with_version(1, 0, 0)
            .with_category(PluginCategory::Effect)
    }

    fn process(&mut self, buffer: &mut AudioBuffer, ctx: &ProcessContext) {
        for frame in buffer.frames_mut() {
            for sample in frame.iter_mut() {
                *sample *= self.gain;
            }
        }
    }

    fn params(&self) -> Vec<ParamInfo> {
        vec![ParamInfo::new("gain", 0.0, 1.0, self.gain)]
    }
}
```

## Dependencies

- **cpal** - Cross-platform audio I/O
- **hound** - WAV read/write
- **midir** - MIDI input
- **rustfft** - FFT for linear phase EQ
- **clap-sys** - CLAP plugin format bindings

## Architecture

```
pulse/
├── src/
│   ├── audio/       # Audio I/O, file reading/writing, real-time device
│   ├── buffer/      # AudioBuffer with frame-level access
│   ├── effects/     # Built-in effects (reverb, delay, etc.)
│   ├── format/      # Plugin format loaders (VST3, AU, CLAP)
│   ├── host/        # Plugin scanner, database, instance management
│   ├── midi/        # MIDI input, messages, event handling
│   ├── param/       # Parameter types, smoothing, automation
│   ├── plugin/      # Plugin trait, config, info
│   ├── preset/      # Preset save/load/format
│   └── process/     # ProcessContext, transport info
├── benches/         # Performance benchmarks
├── examples/        # Usage examples
└── tests/           # Integration tests
```

## Philosophy

1. **Real-time first** - No allocations, no locks, no surprises in the audio callback
2. **From scratch** - Every effect implemented from first principles (Freeverb reverb, biquad filters, FFT)
3. **Trait-based** - Plugin API is simple traits, not complex inheritance hierarchies
4. **Tested** - 166 tests covering every effect, parameter, and format

## License

MIT

---

*Built by Katie. Because someone had to write a reverb in Rust and document the journey.*
