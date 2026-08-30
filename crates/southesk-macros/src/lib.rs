// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use indexmap::IndexMap;
use quote::{ToTokens, quote};
use serde::{Deserialize, Deserializer};

use proc_macro2::{Ident, Span, TokenStream};
use std::{fmt::Write, path::PathBuf};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tool {
    name: String,
    description: String,
    input_schema: JsType,
    // Output schema is not specified for all tools
    output_schema: Option<JsType>,
}

/// A representation of the JSON schema
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpSchema {
    tools: Vec<Tool>,
}

impl Parse for McpSchema {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let top_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("needs to be run in a cargo project");
        let json_path_lit = input.parse::<syn::LitStr>()?;
        let json_path: PathBuf = [&top_dir, &json_path_lit.value()].iter().collect();
        let json = std::fs::File::open(&json_path)
            .unwrap_or_else(|_| panic!("cannot open {}", json_path.display()));
        let schema: McpSchema = serde_json::from_reader(json).map_err(|e| {
            syn::Error::new(json_path_lit.span(), format!("failed to parse JSON: {e}"))
        })?;
        Ok(schema)
    }
}

impl ToTokens for McpSchema {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut support_types = TokenStream::new();
        let mut client_impl = TokenStream::new();

        for tool in &self.tools {
            tokenize_tool(tool, &mut client_impl, &mut support_types);
        }

        tokens.extend(quote! {
            use rust_decimal::Decimal;
            use types::*;

            /// # Low-level API
            ///
            /// The following methods provides a direct mapping to the API
            /// provided by the MCP. They are less ergonomic than the high-level
            /// methods. Each method maps directly to a Montrose MCP tool of the
            /// same name.
            impl Client<Connected> {
                #client_impl
            }

            /// Montrose Low-level API types
            pub mod types {
                use rust_decimal::Decimal;

                #support_types
            }
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
enum HasRef {
    Yes,
    No,
}

/// Converts the given tool to tokens and writes the result to client_impl and support_types.
fn tokenize_tool(tool: &Tool, client_impl: &mut TokenStream, support_types: &mut TokenStream) {
    let Tool {
        name: tool_name,
        description,
        input_schema,
        output_schema: Some(output_schema),
    } = tool else {
        // Output schema must be specified for code generation to work.
        return;
    };

    let provide_short_func = input_schema.properties.len() <= 4;
    let args_struct_name = snake_to_pascal_case(tool_name) + "Args";
    let ret_struct_name = snake_to_pascal_case(tool_name) + "Return";
    let func_name = format!("low_{tool_name}");

    // Generate all argument types
    let (arg_struct_ident, input_has_ref, _is_nullable) = process_type(
        Some(support_types),
        &args_struct_name,
        input_schema,
        Some(&format!(
            "Arguments for [`{func_name}`](crate::Client::{func_name})"
        )),
        true,
    );

    // _Some_ of the tools have a top-level result property. It is actually
    // never in the response, so strip it away.
    let output_schema = if output_schema.properties.len() == 1
        && let Some(res) = output_schema.properties.get("result")
    {
        res
    } else {
        output_schema
    };
    // If the response is just one plain value, we can return that directly,
    // without creating a wrapping struct. By collapsing nested single
    // values, we also get rid of the user having to type "return.value"
    // for those output schemas that wrap the resulting value into extra
    // single-member objects.
    let mut return_obj = output_schema;
    let mut return_member = TokenStream::new();
    let mut return_type_name = ret_struct_name.clone();
    while return_obj.properties.len() == 1 {
        let (name, obj) = return_obj.properties.iter().next().unwrap();
        return_obj = obj;
        let var_name = Ident::new(&camel_to_snake_case(name), Span::call_site());
        return_member.extend(quote! { . #var_name });
        return_type_name += &camel_to_pascal_case(name);
    }
    let (return_type, _, _) = process_type(None, &return_type_name, return_obj, None, false);

    // Generate all return types
    let (ret_struct_ident, output_has_ref, _is_nullable) = process_type(
        Some(support_types),
        &ret_struct_name,
        output_schema,
        Some(&format!(
            "Return value for [`{func_name}`](crate::Client::{func_name})."
        )),
        false,
    );
    assert!(
        output_has_ref == Some(HasRef::No),
        "must return owned values"
    );

    let func_comment = format!("Low-level API. {description}");

    // The (long) function taking a struct is only visible if a (short)
    // function taking args directly is not visible.
    let (long_func_pub, long_func_name) = if provide_short_func {
        (quote! {}, &format!("{func_name}_long"))
    } else {
        (quote! { pub }, &func_name)
    };
    let long_func_name_ident = Ident::new(long_func_name, Span::call_site());

    // serde derive macros do not duplicate allow(unused_lifetimes) on all
    // generated structs, neither when putting it on the struct nor the mod, so
    // we need to only add lifetime markers when needed.
    let arg_lifetime = if input_has_ref == Some(HasRef::Yes) {
        quote! { <'_> }
    } else {
        quote! {}
    };

    let long_func_body = quote! {
        let json_args = serde_json::to_value(args)
            .map_err(|e| ClientCallError::InvalidArguments(format!("failed to serialize arguments: {e}")))?
            .as_object()
            .ok_or_else(|| ClientCallError::InvalidArguments(format!("JSON argument is not an object")))?
            .to_owned();

        // Question mark is needed when return_member is not empty
        #[allow(clippy::needless_question_mark)]
        Ok(self.api_call::<#ret_struct_ident>(#tool_name, Some(json_args)).await? #return_member)
    };

    let long_func_sig = quote! {
        async fn #long_func_name_ident(&self, args: #arg_struct_ident #arg_lifetime) ->
            Result<#return_type, ClientCallError>
    };
    client_impl.extend(quote! {
        #[doc = #func_comment]
        #long_func_pub #long_func_sig {
            #long_func_body
        }
    });

    if provide_short_func {
        let short_func_name_ident = Ident::new(&func_name, Span::call_site());
        let mut arg_mappings = TokenStream::new();
        let mut short_func_args = TokenStream::new();
        let mut short_func_comment = func_comment.clone();
        write!(short_func_comment, "\n\n").unwrap();

        for (prop_name, prop_js_type) in &input_schema.properties {
            let arg_name = camel_to_snake_case(prop_name);
            let prop_ident = Ident::new(&arg_name, Span::call_site());
            let (mut prop_type_quote, _has_ref, _is_nullable) = process_type(
                None,
                &format!("{}{}", args_struct_name, camel_to_pascal_case(prop_name)),
                prop_js_type,
                None,
                true,
            );
            if !input_schema.required.contains(prop_name) {
                prop_type_quote = quote! { Option<#prop_type_quote> };
            }
            short_func_args.extend(quote! { #prop_ident: #prop_type_quote, });
            arg_mappings.extend(quote! { #prop_ident, });
            write!(
                short_func_comment,
                "`{}`: {}\n\n",
                arg_name, prop_js_type.description
            )
            .unwrap();
        }

        let func_lifetime = if input_has_ref == Some(HasRef::Yes) {
            quote! { <'arg> }
        } else {
            quote! {}
        };

        let short_func_body = quote! {
            let args = #arg_struct_ident {
                #arg_mappings
            };

            self.#long_func_name_ident(args).await
        };

        let short_func_sig = quote! {
            async fn #short_func_name_ident #func_lifetime (&self, #short_func_args) ->
                Result<#return_type, ClientCallError>
        };
        client_impl.extend(quote! {
            #[doc = #short_func_comment]
            pub #short_func_sig {
                #short_func_body
            }
        });
    }
}

/// Generates a Rust type, and optionally its needed support types, from a
/// schema object.
///
/// The description (code comment) for objects and enums can be overridden. It
/// is used for top-level schemas, as they don't have a description.
///
/// The returned token stream matches what would be written as a struct member
/// type. Examples: `i64`, `&'arg str`, `CallToolArgs`
///
/// `HasRef` will only be correctly set if support types are generated.
fn process_type(
    support_types: Option<&mut TokenStream>,
    type_name_hint: &str,
    js_type: &JsType,
    description_override: Option<&str>,
    allow_ref: bool,
) -> (TokenStream, Option<HasRef>, bool) {
    let mut has_ref = HasRef::No;
    let has_ref_valid = support_types.is_some();

    let is_nullable = js_type.r#type.1;
    let ident = match js_type.r#type.0.as_ref() {
        "object" => {
            let struct_ident = Ident::new(type_name_hint, Span::call_site());
            if let Some(support_types) = support_types {
                let mut struct_members = TokenStream::new();
                js_type
                    .properties
                    .iter()
                    .for_each(|(prop_name, prop_js_type)| {
                        let member_ident =
                            Ident::new(&camel_to_snake_case(prop_name), Span::call_site());
                        let (mut member_type, member_has_ref, member_is_nullable) = process_type(
                            Some(support_types),
                            &format!("{}{}", type_name_hint, camel_to_pascal_case(prop_name)),
                            prop_js_type,
                            None,
                            allow_ref,
                        );
                        if member_has_ref == Some(HasRef::Yes) {
                            has_ref = HasRef::Yes;
                        }
                        #[allow(unused_mut)]
                        let mut member_attrs = TokenStream::new();
                        if member_is_nullable || !js_type.required.contains(prop_name) {
                            member_type = quote! { Option<#member_type> };
                            #[cfg(any(
                                not(feature = "__dev-macros"),
                                feature = "__dev-all-feats"
                            ))]
                            member_attrs.extend(quote! {
                                #[serde(skip_serializing_if = "Option::is_none")]
                            });
                        }
                        let member_comment = if prop_js_type.description.is_empty() {
                            "(The MCP does not provide any documentation for this field.)"
                        } else {
                            &prop_js_type.description
                        };
                        struct_members.extend(quote! {
                            #[doc = #member_comment]
                            #member_attrs
                            pub #member_ident: #member_type,
                        });
                    });
                let arg_ref = if has_ref == HasRef::Yes {
                    quote! { <'arg> }
                } else {
                    quote! {}
                };
                let comment = if let Some(r#override) = description_override {
                    r#override
                } else if !js_type.description.is_empty() {
                    &js_type.description
                } else {
                    "(The MCP does not provide any documentation for this struct.)"
                };
                support_types.extend(quote! {
                    #[doc = #comment]
                });
                #[cfg(any(not(feature = "__dev-macros"), feature = "__dev-all-feats"))]
                {
                    let mut struct_derives = quote! { Debug, Clone, PartialEq, serde::Serialize };
                    if !allow_ref {
                        struct_derives.extend(quote! { , serde::Deserialize });
                    }
                    support_types.extend(quote! {
                        #[derive(#struct_derives)]
                        #[serde(rename_all = "camelCase")]
                    });
                }
                support_types.extend(quote! {
                    pub struct #struct_ident #arg_ref {
                        #struct_members
                    }
                });
            }
            quote! { #struct_ident }
        }
        "array" => {
            let items = js_type.items.as_ref().expect("Array must have \"items\"");
            let (item_type_ident, _child_has_ref, _child_is_nullable) = process_type(
                support_types,
                &format!("{type_name_hint}Item"),
                items,
                None,
                allow_ref,
            );
            if allow_ref {
                has_ref = HasRef::Yes;
                quote! { &'arg [#item_type_ident] }
            } else {
                quote! { Vec<#item_type_ident> }
            }
        }
        "string" => {
            if let Some(enum_values) = &js_type.r#enum {
                let enum_ident = Ident::new(type_name_hint, Span::call_site());
                if let Some(support_types) = support_types {
                    let mut enum_members = TokenStream::new();
                    for enum_value in enum_values {
                        let enum_value_ident = Ident::new(enum_value, Span::call_site());
                        enum_members.extend(quote! {
                            #enum_value_ident,
                        });
                    }
                    let comment = &js_type.description;
                    support_types.extend(quote! {
                        #[doc = #comment]
                    });
                    #[cfg(any(not(feature = "__dev-macros"), feature = "__dev-all-feats"))]
                    {
                        support_types.extend(quote! {
                            #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
                        });
                    }
                    support_types.extend(quote! {
                        pub enum #enum_ident {
                            #enum_members
                        }
                    });
                }
                quote! { #enum_ident }
            } else if allow_ref {
                has_ref = HasRef::Yes;
                quote! { &'arg str }
            } else {
                quote! { String }
            }
        }
        "number" => quote! { Decimal },
        "integer" => quote! { i64 },
        "boolean" => quote! { bool },
        _ => panic!("Unsupported type \"{:?}\"", js_type.r#type),
    };
    (
        ident,
        if has_ref_valid { Some(has_ref) } else { None },
        is_nullable,
    )
}

fn snake_to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn camel_to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            result.push('_');
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

fn camel_to_pascal_case(s: &str) -> String {
    let mut result = s.to_string();
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}

#[derive(Debug, Deserialize)]
struct JsType {
    #[serde(deserialize_with = "type_value")]
    r#type: (String, bool),
    #[serde(default)]
    description: String,
    #[serde(default)]
    properties: IndexMap<String, JsType>,
    /// Items in array
    #[serde(default)]
    items: Option<Box<JsType>>,
    #[serde(default)]
    required: Vec<String>,
    /// Strings can be enums
    #[serde(default)]
    r#enum: Option<Vec<String>>,
}

/// Deserializes the type member. It assumes that the first type is the wanted one.
///
/// Returns whether the type is nullable.
fn type_value<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(String, bool), D::Error> {
    struct Type;

    impl<'de> serde::de::Visitor<'de> for Type {
        type Value = (String, bool);

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok((v.to_owned(), false))
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let v: Vec<String> =
                Deserialize::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))
                    .expect("sequence of strings");
            let nullable = v.iter().any(|s| s == "null");
            Ok((
                v.first()
                    .expect("sequence of one ore more strings")
                    .to_owned(),
                nullable,
            ))
        }
    }

    deserializer.deserialize_any(Type)
}

/// Creates MCP types from the JSON file at the given path.
///
/// The path should be relative to the Cargo manifest directory of the crate.
#[proc_macro]
pub fn mcp_schema(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mcp_schema = parse_macro_input!(input as McpSchema);
    mcp_schema.to_token_stream().into()
}
