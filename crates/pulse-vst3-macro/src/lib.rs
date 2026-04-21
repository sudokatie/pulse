//! VST3 plugin procedural macro
//!
//! Provides the `#[vst3_plugin]` attribute macro for generating VST3 plugin
//! export code from a Pulse plugin struct.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Attribute macro for generating VST3 plugin export code.
///
/// This macro generates the necessary boilerplate for exporting a Pulse plugin
/// as a VST3 plugin, including:
/// - Factory registration
/// - VSTPluginMain entry point (GetPluginFactory)
/// - Platform-specific module init/exit functions
/// - Bundle metadata struct
///
/// # Usage
///
/// ```ignore
/// #[vst3_plugin(name = "MyEffect", vendor = "Katie", category = "Fx")]
/// struct MyEffect {
///     // plugin fields
/// }
/// ```
///
/// # Attributes
///
/// - `name`: The display name of the plugin (required)
/// - `vendor`: The plugin vendor/manufacturer (required)
/// - `category`: Plugin category - one of "Fx", "Instrument", "Analyzer", "Generator" (default: "Fx")
/// - `id`: Unique plugin ID (default: generated from name)
/// - `version`: Plugin version string (default: "1.0.0")
/// - `url`: Vendor URL (default: "")
/// - `email`: Support email (default: "")
/// - `inputs`: Number of audio inputs (default: 2)
/// - `outputs`: Number of audio outputs (default: 2)
#[proc_macro_attribute]
pub fn vst3_plugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let attrs = parse_vst3_attrs(attr.into());

    let expanded = generate_vst3_export(&input, &attrs);

    expanded.into()
}

/// Parsed VST3 plugin attributes
struct Vst3Attrs {
    name: String,
    vendor: String,
    category: String,
    id: Option<String>,
    version: String,
    url: String,
    email: String,
    inputs: usize,
    outputs: usize,
}

impl Default for Vst3Attrs {
    fn default() -> Self {
        Self {
            name: String::new(),
            vendor: String::new(),
            category: "Fx".to_string(),
            id: None,
            version: "1.0.0".to_string(),
            url: String::new(),
            email: String::new(),
            inputs: 2,
            outputs: 2,
        }
    }
}

/// Helper struct for parsing attribute arguments
struct AttrArgs(syn::punctuated::Punctuated<syn::MetaNameValue, syn::Token![,]>);

impl syn::parse::Parse for AttrArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let items = syn::punctuated::Punctuated::parse_terminated(input)?;
        Ok(AttrArgs(items))
    }
}

fn parse_vst3_attrs(tokens: TokenStream2) -> Vst3Attrs {
    let mut attrs = Vst3Attrs::default();

    // Parse each key=value pair from the punctuated list
    if tokens.is_empty() {
        return attrs;
    }

    // Parse as punctuated meta items
    let parsed: Result<AttrArgs, _> = syn::parse2(tokens);

    if let Ok(AttrArgs(items)) = parsed {
        for item in items {
            let ident = item.path.get_ident().map(|i| i.to_string());
            if let Some(key) = ident {
                match key.as_str() {
                    "name" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.name = s.value();
                        }
                    }
                    "vendor" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.vendor = s.value();
                        }
                    }
                    "category" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.category = s.value();
                        }
                    }
                    "id" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.id = Some(s.value());
                        }
                    }
                    "version" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.version = s.value();
                        }
                    }
                    "url" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.url = s.value();
                        }
                    }
                    "email" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &item.value {
                            attrs.email = s.value();
                        }
                    }
                    "inputs" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) = &item.value {
                            if let Ok(n) = i.base10_parse() {
                                attrs.inputs = n;
                            }
                        }
                    }
                    "outputs" => {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) = &item.value {
                            if let Ok(n) = i.base10_parse() {
                                attrs.outputs = n;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    attrs
}

fn generate_vst3_export(input: &DeriveInput, attrs: &Vst3Attrs) -> TokenStream2 {
    let struct_name = &input.ident;
    let vis = &input.vis;

    // Generate plugin ID if not provided
    let plugin_id = attrs.id.clone().unwrap_or_else(|| {
        format!("com.pulse.{}", attrs.name.to_lowercase().replace(' ', "-"))
    });

    let name = &attrs.name;
    let vendor = &attrs.vendor;
    let version = &attrs.version;
    let url = &attrs.url;
    let email = &attrs.email;
    let inputs = attrs.inputs;
    let outputs = attrs.outputs;

    // Map category string to PluginCategory
    let category_ident = match attrs.category.to_lowercase().as_str() {
        "fx" | "effect" => quote! { ::pulse::plugin::PluginCategory::Effect },
        "instrument" | "synth" => quote! { ::pulse::plugin::PluginCategory::Instrument },
        "analyzer" => quote! { ::pulse::plugin::PluginCategory::Analyzer },
        "generator" => quote! { ::pulse::plugin::PluginCategory::Generator },
        _ => quote! { ::pulse::plugin::PluginCategory::Other },
    };

    // Generate the metadata struct
    let metadata_name = syn::Ident::new(
        &format!("{}Vst3Metadata", struct_name),
        struct_name.span(),
    );

    quote! {
        // Original struct definition
        #input

        /// VST3 bundle metadata for this plugin
        #vis struct #metadata_name;

        impl #metadata_name {
            /// Plugin name
            pub const NAME: &'static str = #name;
            /// Plugin vendor
            pub const VENDOR: &'static str = #vendor;
            /// Plugin ID
            pub const ID: &'static str = #plugin_id;
            /// Plugin version
            pub const VERSION: &'static str = #version;
            /// Vendor URL
            pub const URL: &'static str = #url;
            /// Support email
            pub const EMAIL: &'static str = #email;
            /// Number of audio inputs
            pub const INPUTS: usize = #inputs;
            /// Number of audio outputs
            pub const OUTPUTS: usize = #outputs;

            /// Get plugin info
            pub fn info() -> ::pulse::plugin::PluginInfo {
                ::pulse::plugin::PluginInfo {
                    id: Self::ID.to_string(),
                    name: Self::NAME.to_string(),
                    vendor: Self::VENDOR.to_string(),
                    version: Self::VERSION.to_string(),
                    category: #category_ident,
                    inputs: Self::INPUTS,
                    outputs: Self::OUTPUTS,
                }
            }
        }

        /// VST3 entry point - returns the plugin factory
        #[no_mangle]
        pub unsafe extern "C" fn GetPluginFactory() -> *mut ::std::ffi::c_void {
            use ::pulse::export::factory::{FactoryInfo, Vst3PluginFactory, generate_tuid};
            use ::pulse::plugin::Plugin;

            let info = FactoryInfo {
                vendor: #vendor.to_string(),
                url: #url.to_string(),
                email: #email.to_string(),
            };

            let mut factory = Vst3PluginFactory::new(info);

            let create_plugin = || -> Box<dyn Plugin> {
                Box::new(<#struct_name>::default())
            };

            let plugin = create_plugin();
            let plugin_info = plugin.info();
            drop(plugin);

            let processor_cid = generate_tuid(&plugin_info.id, "processor");
            let controller_cid = generate_tuid(&plugin_info.id, "controller");

            factory.register_processor(
                processor_cid,
                controller_cid,
                &plugin_info,
                move || Box::new(<#struct_name>::default()),
            );

            factory.register_controller(
                controller_cid,
                &plugin_info,
                move || Box::new(<#struct_name>::default()),
            );

            Box::into_raw(factory) as *mut ::std::ffi::c_void
        }

        /// Windows DLL init
        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub unsafe extern "system" fn InitDll() -> bool {
            true
        }

        /// Windows DLL exit
        #[cfg(target_os = "windows")]
        #[no_mangle]
        pub unsafe extern "system" fn ExitDll() -> bool {
            true
        }

        /// macOS/Linux module entry
        #[cfg(not(target_os = "windows"))]
        #[no_mangle]
        pub unsafe extern "C" fn ModuleEntry(_: *mut ::std::ffi::c_void) -> bool {
            true
        }

        /// macOS/Linux module exit
        #[cfg(not(target_os = "windows"))]
        #[no_mangle]
        pub unsafe extern "C" fn ModuleExit() -> bool {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_attrs() {
        let tokens = quote! {};
        let attrs = parse_vst3_attrs(tokens);

        assert_eq!(attrs.category, "Fx");
        assert_eq!(attrs.version, "1.0.0");
        assert_eq!(attrs.inputs, 2);
        assert_eq!(attrs.outputs, 2);
    }

    #[test]
    fn test_default_attrs() {
        let attrs = Vst3Attrs::default();

        assert!(attrs.name.is_empty());
        assert!(attrs.vendor.is_empty());
        assert_eq!(attrs.category, "Fx");
        assert!(attrs.id.is_none());
        assert_eq!(attrs.version, "1.0.0");
    }
}
