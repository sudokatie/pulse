//! Pulse PluginCategory to VST3 category mapping

use crate::plugin::PluginCategory;
use super::types::categories;

/// Convert a Pulse PluginCategory to VST3 category string
pub fn category_to_vst3(category: PluginCategory) -> &'static str {
    match category {
        PluginCategory::Effect => categories::FX,
        PluginCategory::Instrument => categories::INSTRUMENT,
        PluginCategory::Analyzer => categories::FX_ANALYZER,
        PluginCategory::Generator => categories::FX_GENERATOR,
        PluginCategory::Other => categories::FX_TOOLS,
    }
}

/// Convert VST3 category string to Pulse PluginCategory
pub fn vst3_to_category(vst3_category: &str) -> PluginCategory {
    let lower = vst3_category.to_lowercase();

    if lower.contains("instrument") || lower.contains("synth") || lower.contains("sampler") {
        PluginCategory::Instrument
    } else if lower.contains("analyzer") {
        PluginCategory::Analyzer
    } else if lower.contains("generator") {
        PluginCategory::Generator
    } else if lower.starts_with("fx") || lower.contains("effect") {
        PluginCategory::Effect
    } else {
        PluginCategory::Other
    }
}

/// Get VST3 subcategory string for a plugin based on its category and hints
pub fn get_subcategories(category: PluginCategory, hints: &[&str]) -> String {
    let mut subcats = vec![category_to_vst3(category)];

    for hint in hints {
        let hint_lower = hint.to_lowercase();
        let subcat = match hint_lower.as_str() {
            "reverb" => Some(categories::FX_REVERB),
            "delay" => Some(categories::FX_DELAY),
            "distortion" | "overdrive" | "saturation" => Some(categories::FX_DISTORTION),
            "compressor" | "limiter" | "gate" | "dynamics" => Some(categories::FX_DYNAMICS),
            "eq" | "equalizer" | "filter" => Some(categories::FX_EQ),
            "chorus" | "flanger" | "phaser" | "modulation" => Some(categories::FX_MODULATION),
            "spatial" | "panner" | "stereo" => Some(categories::FX_SPATIAL),
            "synth" | "synthesizer" => Some(categories::INSTRUMENT_SYNTH),
            "sampler" => Some(categories::INSTRUMENT_SAMPLER),
            "drum" | "drums" => Some(categories::INSTRUMENT_DRUM),
            _ => None,
        };

        if let Some(s) = subcat {
            if !subcats.contains(&s) {
                subcats.push(s);
            }
        }
    }

    subcats.join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_to_vst3() {
        assert_eq!(category_to_vst3(PluginCategory::Effect), "Fx");
        assert_eq!(category_to_vst3(PluginCategory::Instrument), "Instrument");
        assert_eq!(category_to_vst3(PluginCategory::Analyzer), "Fx|Analyzer");
        assert_eq!(category_to_vst3(PluginCategory::Generator), "Fx|Generator");
        assert_eq!(category_to_vst3(PluginCategory::Other), "Fx|Tools");
    }

    #[test]
    fn test_vst3_to_category() {
        // Instruments
        assert_eq!(vst3_to_category("Instrument"), PluginCategory::Instrument);
        assert_eq!(vst3_to_category("Instrument|Synth"), PluginCategory::Instrument);
        assert_eq!(vst3_to_category("Instrument|Sampler"), PluginCategory::Instrument);

        // Effects
        assert_eq!(vst3_to_category("Fx"), PluginCategory::Effect);
        assert_eq!(vst3_to_category("Fx|Reverb"), PluginCategory::Effect);
        assert_eq!(vst3_to_category("Fx|Delay"), PluginCategory::Effect);

        // Analyzers
        assert_eq!(vst3_to_category("Fx|Analyzer"), PluginCategory::Analyzer);

        // Generators
        assert_eq!(vst3_to_category("Fx|Generator"), PluginCategory::Generator);

        // Unknown
        assert_eq!(vst3_to_category("Unknown"), PluginCategory::Other);
    }

    #[test]
    fn test_vst3_to_category_case_insensitive() {
        assert_eq!(vst3_to_category("INSTRUMENT"), PluginCategory::Instrument);
        assert_eq!(vst3_to_category("fx"), PluginCategory::Effect);
        assert_eq!(vst3_to_category("FX|ANALYZER"), PluginCategory::Analyzer);
    }

    #[test]
    fn test_category_roundtrip() {
        for category in [
            PluginCategory::Effect,
            PluginCategory::Instrument,
            PluginCategory::Analyzer,
            PluginCategory::Generator,
        ] {
            let vst3 = category_to_vst3(category);
            let back = vst3_to_category(vst3);
            assert_eq!(back, category);
        }
    }

    #[test]
    fn test_get_subcategories_basic() {
        let subcats = get_subcategories(PluginCategory::Effect, &[]);
        assert_eq!(subcats, "Fx");
    }

    #[test]
    fn test_get_subcategories_with_hints() {
        let subcats = get_subcategories(PluginCategory::Effect, &["reverb"]);
        assert!(subcats.contains("Fx"));
        assert!(subcats.contains("Fx|Reverb"));

        let subcats = get_subcategories(PluginCategory::Effect, &["delay", "modulation"]);
        assert!(subcats.contains("Fx|Delay"));
        assert!(subcats.contains("Fx|Modulation"));
    }

    #[test]
    fn test_get_subcategories_instrument() {
        let subcats = get_subcategories(PluginCategory::Instrument, &["synth"]);
        assert!(subcats.contains("Instrument"));
        assert!(subcats.contains("Instrument|Synth"));

        let subcats = get_subcategories(PluginCategory::Instrument, &["sampler", "drum"]);
        assert!(subcats.contains("Instrument|Sampler"));
        assert!(subcats.contains("Instrument|Drum"));
    }

    #[test]
    fn test_get_subcategories_no_duplicates() {
        let subcats = get_subcategories(PluginCategory::Effect, &["reverb", "reverb"]);
        // Should only have "Fx" and "Fx|Reverb" once each
        // The string should be "Fx|Fx|Reverb" not "Fx|Fx|Reverb|Fx|Reverb"
        assert_eq!(subcats, "Fx|Fx|Reverb");
    }

    #[test]
    fn test_get_subcategories_case_insensitive_hints() {
        let subcats = get_subcategories(PluginCategory::Effect, &["REVERB"]);
        assert!(subcats.contains("Fx|Reverb"));

        let subcats = get_subcategories(PluginCategory::Effect, &["Delay"]);
        assert!(subcats.contains("Fx|Delay"));
    }

    #[test]
    fn test_dynamics_hints() {
        let subcats = get_subcategories(PluginCategory::Effect, &["compressor"]);
        assert!(subcats.contains("Fx|Dynamics"));

        let subcats = get_subcategories(PluginCategory::Effect, &["limiter"]);
        assert!(subcats.contains("Fx|Dynamics"));

        let subcats = get_subcategories(PluginCategory::Effect, &["gate"]);
        assert!(subcats.contains("Fx|Dynamics"));
    }

    #[test]
    fn test_distortion_hints() {
        let subcats = get_subcategories(PluginCategory::Effect, &["distortion"]);
        assert!(subcats.contains("Fx|Distortion"));

        let subcats = get_subcategories(PluginCategory::Effect, &["overdrive"]);
        assert!(subcats.contains("Fx|Distortion"));

        let subcats = get_subcategories(PluginCategory::Effect, &["saturation"]);
        assert!(subcats.contains("Fx|Distortion"));
    }
}
