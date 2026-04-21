//! Pulse CLI - Audio plugin framework

use clap::{Parser, Subcommand};
use pulse::audio::{read_audio_file, write_audio_file, AudioDevice, AudioStream};
use pulse::effects::{Reverb, Delay, Compressor, ParametricEQ, Distortion};
use pulse::host::{PluginDatabase, PluginScanner, PluginFormat};
use pulse::plugin::Plugin;
use pulse::process::ProcessContext;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "pulse")]
#[command(about = "Audio plugin framework - host plugins and process effects")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List and manage plugins
    Plugins {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Process audio with a plugin or effect
    Process {
        /// Input audio file
        input: String,
        /// Output audio file
        output: String,
        /// Plugin ID
        #[arg(long)]
        plugin: Option<String>,
        /// Built-in effect (reverb, delay, compressor, eq, distortion)
        #[arg(long)]
        effect: Option<String>,
        /// Parameters as key=value pairs
        #[arg(long, value_parser = parse_param)]
        param: Vec<(String, f32)>,
    },
    /// Run a built-in effect
    Effect {
        /// Effect type (reverb, delay, compressor, eq, distortion)
        effect_type: String,
        /// Input audio file
        input: String,
        /// Output audio file
        output: String,
        /// Parameters as key=value pairs
        #[arg(long, value_parser = parse_param)]
        param: Vec<(String, f32)>,
    },
    /// Host a plugin with real-time audio
    Host {
        /// Plugin ID or effect name
        plugin_id: String,
        /// MIDI input device
        #[arg(long)]
        midi_input: Option<String>,
        /// Audio output device
        #[arg(long)]
        audio_output: Option<String>,
    },
    /// Package a plugin as a VST3 bundle
    Package {
        /// Plugin name
        #[arg(long)]
        name: String,
        /// Plugin ID (e.g., com.example.myplugin)
        #[arg(long)]
        id: String,
        /// Vendor name
        #[arg(long)]
        vendor: String,
        /// Version string (e.g., 1.0.0)
        #[arg(long, default_value = "1.0.0")]
        version: String,
        /// Source binary path (compiled dylib/so/dll)
        #[arg(long)]
        binary: Option<String>,
        /// Output directory
        #[arg(long, short, default_value = ".")]
        output: String,
        /// Target platform (macos, linux, windows)
        #[arg(long)]
        platform: Option<String>,
    },
    /// Install a VST3 bundle to the system plugin directory
    Install {
        /// Path to the VST3 bundle
        bundle: String,
        /// Target directory (defaults to system VST3 directory)
        #[arg(long)]
        target: Option<String>,
    },
    /// Validate a VST3 bundle structure
    Validate {
        /// Path to the VST3 bundle
        bundle: String,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List available plugins
    List {
        /// Filter by format (vst3, au, clap)
        #[arg(long)]
        format: Option<String>,
    },
    /// Scan for plugins
    Scan {
        /// Additional search path
        #[arg(long)]
        path: Option<String>,
    },
    /// Show plugin info
    Info {
        /// Plugin ID
        plugin_id: String,
    },
}

fn parse_param(s: &str) -> Result<(String, f32), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid parameter format: {}", s));
    }
    let value: f32 = parts[1].parse().map_err(|_| format!("Invalid value: {}", parts[1]))?;
    Ok((parts[0].to_string(), value))
}

fn create_effect(name: &str, sample_rate: u32) -> Option<Box<dyn Plugin>> {
    match name.to_lowercase().as_str() {
        "reverb" => Some(Box::new(Reverb::new(sample_rate))),
        "delay" => Some(Box::new(Delay::new(sample_rate))),
        "compressor" | "comp" => Some(Box::new(Compressor::new(sample_rate))),
        "eq" | "parametriceq" => Some(Box::new(ParametricEQ::new(sample_rate))),
        "distortion" | "dist" => Some(Box::new(Distortion::new(sample_rate))),
        _ => None,
    }
}

fn apply_params(effect: &mut dyn Plugin, params: &[(String, f32)]) {
    for (name, value) in params {
        // Map common parameter names to IDs
        let id = match name.to_lowercase().as_str() {
            // Reverb
            "room" | "size" | "room_size" => 0,
            "damp" | "damping" => 1,
            "wet" | "mix" => 2,
            "width" => 3,
            // Delay
            "time" | "delay_time" => 0,
            "feedback" => 1,
            "pingpong" | "ping_pong" => 3,
            // Compressor
            "threshold" | "thresh" => 0,
            "ratio" => 1,
            "attack" => 2,
            "release" => 3,
            "knee" => 4,
            "makeup" => 5,
            // EQ
            "band0_freq" | "low_freq" => 0,
            "band0_gain" | "low_gain" => 1,
            "band0_q" | "low_q" => 2,
            "band1_freq" | "mid_freq" => 3,
            "band1_gain" | "mid_gain" => 4,
            "band1_q" | "mid_q" => 5,
            "band2_freq" | "high_freq" => 6,
            "band2_gain" | "high_gain" => 7,
            "band2_q" | "high_q" => 8,
            "linear_phase" => 100,
            // Distortion
            "drive" => 0,
            "tone" => 1,
            // Generic numbered params
            name if name.starts_with("p") => {
                name[1..].parse().unwrap_or(0)
            }
            _ => continue,
        };
        effect.set_parameter(id, *value);
    }
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Plugins { action } => handle_plugins(action),
        Commands::Process { input, output, plugin, effect, param } => {
            handle_process(&input, &output, plugin.as_deref(), effect.as_deref(), &param);
        }
        Commands::Effect { effect_type, input, output, param } => {
            handle_effect(&effect_type, &input, &output, &param);
        }
        Commands::Host { plugin_id, midi_input, audio_output } => {
            handle_host(&plugin_id, midi_input.as_deref(), audio_output.as_deref());
        }
        Commands::Package { name, id, vendor, version, binary, output, platform } => {
            handle_package(&name, &id, &vendor, &version, binary.as_deref(), &output, platform.as_deref());
        }
        Commands::Install { bundle, target } => {
            handle_install(&bundle, target.as_deref());
        }
        Commands::Validate { bundle } => {
            handle_validate(&bundle);
        }
    }
}

fn handle_plugins(action: PluginAction) {
    let db_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pulse")
        .join("plugins.json");
    
    let mut database = PluginDatabase::load(&db_path).unwrap_or_else(|_| PluginDatabase::new());
    
    match action {
        PluginAction::List { format } => {
            let format_filter = format.as_ref().and_then(|f| match f.to_lowercase().as_str() {
                "vst3" => Some(PluginFormat::Vst3),
                "au" | "audiounit" => Some(PluginFormat::AudioUnit),
                "clap" => Some(PluginFormat::Clap),
                _ => None,
            });
            
            let plugins: Vec<_> = if let Some(fmt) = format_filter {
                database.filter_by_format(fmt).cloned().collect()
            } else {
                database.all_plugins().cloned().collect()
            };
            
            if plugins.is_empty() {
                println!("No plugins found. Run 'pulse plugins scan' to discover plugins.");
            } else {
                println!("Found {} plugins:", plugins.len());
                for plugin in plugins {
                    println!("  {} - {} [{}]", plugin.id, plugin.name, plugin.format.extension());
                }
            }
        }
        PluginAction::Scan { path } => {
            println!("Scanning for plugins...");
            
            let mut scanner = PluginScanner::new();
            if let Some(p) = path {
                scanner.add_search_path(PathBuf::from(p));
            }
            
            let plugins = scanner.scan_all();
            let count = plugins.len();
            
            for plugin in plugins {
                database.add_or_update(plugin.into());
            }
            
            // Save database
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = database.save(&db_path) {
                eprintln!("Warning: Could not save database: {}", e);
            }
            
            println!("Found {} plugins", count);
        }
        PluginAction::Info { plugin_id } => {
            if let Some(plugin) = database.find_by_id(&plugin_id) {
                println!("Plugin: {}", plugin.name);
                println!("  ID: {}", plugin.id);
                println!("  Vendor: {}", plugin.vendor);
                println!("  Format: {}", plugin.format.extension());
                println!("  Path: {}", plugin.path.display());
                println!("  Inputs: {}", plugin.inputs);
                println!("  Outputs: {}", plugin.outputs);
                if let Some(cat) = &plugin.category {
                    println!("  Category: {}", cat);
                }
            } else {
                eprintln!("Plugin not found: {}", plugin_id);
                eprintln!("Run 'pulse plugins list' to see available plugins.");
            }
        }
    }
}

fn handle_process(input: &str, output: &str, plugin: Option<&str>, effect: Option<&str>, params: &[(String, f32)]) {
    // Read input file
    let (mut buffer, info) = match read_audio_file(input) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error reading input file: {}", e);
            return;
        }
    };
    
    println!("Processing {} ({} channels, {} Hz, {} samples)", 
             input, info.channels, info.sample_rate, info.duration_samples);
    
    let ctx = ProcessContext {
        sample_rate: info.sample_rate as f32,
        block_size: buffer.frames(),
        ..Default::default()
    };
    
    // Create effect or load plugin
    if let Some(effect_name) = effect {
        if let Some(mut fx) = create_effect(effect_name, info.sample_rate) {
            apply_params(fx.as_mut(), params);
            fx.process(&mut buffer, &ctx);
            println!("Applied {} effect", effect_name);
        } else {
            eprintln!("Unknown effect: {}", effect_name);
            return;
        }
    } else if let Some(_plugin_id) = plugin {
        eprintln!("External plugin loading not yet implemented");
        eprintln!("Use --effect for built-in effects: reverb, delay, compressor, eq, distortion");
        return;
    } else {
        eprintln!("No effect or plugin specified");
        return;
    }
    
    // Write output
    if let Err(e) = write_audio_file(output, &buffer, info.sample_rate) {
        eprintln!("Error writing output file: {}", e);
        return;
    }
    
    println!("Wrote {}", output);
}

fn handle_effect(effect_type: &str, input: &str, output: &str, params: &[(String, f32)]) {
    handle_process(input, output, None, Some(effect_type), params);
}

fn handle_package(
    name: &str,
    id: &str,
    vendor: &str,
    version: &str,
    binary: Option<&str>,
    output: &str,
    platform: Option<&str>,
) {
    use pulse::cli::package::{PackageOptions, build_package, parse_platform, print_result};

    let platform = platform.and_then(|p| {
        let parsed = parse_platform(p);
        if parsed.is_none() {
            eprintln!("Warning: Unknown platform '{}', using current platform", p);
        }
        parsed
    });

    let options = PackageOptions {
        name: name.to_string(),
        id: id.to_string(),
        vendor: vendor.to_string(),
        version: version.to_string(),
        binary: binary.map(PathBuf::from),
        output_dir: PathBuf::from(output),
        platform,
    };

    match build_package(options) {
        Ok(result) => {
            print_result(&result);
        }
        Err(e) => {
            eprintln!("Error building package: {}", e);
        }
    }
}

fn handle_host(plugin_id: &str, midi_input: Option<&str>, audio_output: Option<&str>) {
    // Check if it's a built-in effect
    let effect = create_effect(plugin_id, 44100);
    
    if effect.is_none() {
        eprintln!("Plugin/effect not found: {}", plugin_id);
        eprintln!("Available built-in effects: reverb, delay, compressor, eq, distortion");
        return;
    }
    
    let mut effect = effect.unwrap();
    
    // Setup audio device
    let device = match audio_output {
        Some(name) => match AudioDevice::open_by_name(name) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error opening audio device: {}", e);
                return;
            }
        },
        None => match AudioDevice::open_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error opening default audio device: {}", e);
                return;
            }
        }
    };
    
    let sample_rate = device.sample_rate();
    let channels = device.channels();
    
    println!("Audio: {} Hz, {} channels", sample_rate, channels);
    
    // Setup MIDI if requested
    let _midi_manager = if let Some(midi_name) = midi_input {
        let mut manager = pulse::midi::MidiInputManager::new();
        match manager.connect(midi_name) {
            Ok(()) => {
                println!("MIDI: Connected to {}", midi_name);
                Some(manager)
            }
            Err(e) => {
                eprintln!("Warning: Could not connect to MIDI device: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Reinitialize effect at device sample rate
    effect = create_effect(plugin_id, sample_rate).unwrap();
    
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");
    
    println!("\nHosting {} - Press Ctrl-C to stop", plugin_id);
    
    // For demo: generate a simple tone and process through effect
    let mut phase = 0.0f32;
    let freq = 440.0;
    let mut buffer = vec![0.0f32; 1024];
    let ctx = ProcessContext {
        sample_rate: sample_rate as f32,
        block_size: 512,
        ..Default::default()
    };
    
    let stream = AudioStream::new(&device, move |data: &mut [f32]| {
        // Generate test tone
        for sample in data.chunks_mut(channels as usize) {
            let s = (phase * std::f32::consts::TAU).sin() * 0.3;
            phase += freq / sample_rate as f32;
            if phase > 1.0 { phase -= 1.0; }
            
            for ch in sample.iter_mut() {
                *ch = s;
            }
        }
    });
    
    match stream {
        Ok(_stream) => {
            while running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }
            println!("\nStopping...");
        }
        Err(e) => {
            eprintln!("Error starting audio stream: {}", e);
        }
    }
}

fn handle_install(bundle: &str, target: Option<&str>) {
    use pulse::cli::install::{install_bundle, print_install_result};

    let bundle_path = PathBuf::from(bundle);
    let target_path = target.map(PathBuf::from);

    let result = install_bundle(&bundle_path, target_path.as_deref());
    print_install_result(&result);

    if !result.success {
        std::process::exit(1);
    }
}

fn handle_validate(bundle: &str) {
    use pulse::cli::install::{validate_bundle, print_validation_result};

    let bundle_path = PathBuf::from(bundle);
    let result = validate_bundle(&bundle_path);
    print_validation_result(&bundle_path, &result);

    if !result.is_valid() && !result.structure_valid() {
        std::process::exit(1);
    }
}
