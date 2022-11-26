use crate::{parse::Endianness, Format, Item, Repetition, Condition};
use itertools::Itertools;
use proc_macro2::TokenTree;
use proc_macro_error::abort;
use quote::{format_ident, quote, ToTokens};
use syn::{Type, TypePath};

const RUST_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64",
];

/// Creates simple read code for the following 3 cases:
///     - Simple rust types like u16 where can just call reader function with correct endianness
///     - Booleans where need to do a simple conversion
///     - Composite types where we simply call the correct function
fn handle_simple_read(data_type: &syn::Type, endianness: Endianness) -> proc_macro2::TokenStream {
    // need to check if type is existing rust type or custom
    if RUST_TYPES.contains(&data_type.to_token_stream().to_string().as_str()) {
        // simple case where reader code exists, can just reader::read_<type>();
        let fn_call = format_ident!("read_{}", data_type.to_token_stream().to_string());

        match endianness {
            Endianness::Little => {
                quote! {  reader.#fn_call::<::byteorder::LittleEndian>().ok() }
            }
            Endianness::Big => {
                quote! { reader.#fn_call::<::byteorder::BigEndian>().ok() }
            }
        }
    } else if data_type.to_token_stream().to_string() == "bool" {
        quote! { reader.read_u8().map(|i| i != 0).ok() }
    } else {
        // more complex case where needs to use custom implementation
        // e.g. <type>::read(&reader);

        quote! { #data_type::read(reader, &_root) }
    }
}

/// Generates a conditional read from the arguments given.
/// If optional is true, the read pointer will be advanced by the amount of bytes that would be otherwise read.
fn generate_conditional_read(condition: &Condition, read: proc_macro2::TokenStream, data_type: &syn::Type) -> proc_macro2::TokenStream {
    let else_body = if condition.advance_if_false {
        quote! {
            reader.read_exact(&mut [0u8; std::mem::size_of::<#data_type>()]).ok();
            None
        }
    } else {
        quote! {
            None
        }
    };

    let expr = &condition.expression;
    quote! { 
        if #expr {
            Some(#read)
        } else {
            #else_body
        }
    }
}

/// Generates a repeated read from the arguments given.
fn generate_repeated_read(repetition: &Repetition, read: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    match repetition {
        Repetition::Count(expr) => {
            quote! {
                (0..#expr).map(|_| #read).collect::<Option<Vec<_>>>()
            }
        },
    }
}

/// Generates a vector of variable assignments that read the correct type from a reader.
fn generate_read_calls(items: &[Item], endianness: Endianness, struct_name: &syn::Ident) -> Vec<proc_macro2::TokenStream> {
    items.iter().map(|item| {
        let Item {
            id,
            data_type,
            condition,
            repetition
        } = item;

        if let Type::Path(TypePath { path, .. }) = data_type && path.segments.first().map(|x| x.ident != "Option").unwrap_or(true) {
            // simplest case, no option
            // generate the applicable read code
            let mut read = handle_simple_read(data_type, endianness);

            // if conditional, update with required code
            if let Some(condition) = condition {
                read = generate_conditional_read(condition, read, data_type);
            }
            // same for repetition
            if let Some(repetition) = repetition {
                read = generate_repeated_read(repetition, read);
            }

            quote! { let #id = #read? }
        } else if let Type::Path(TypePath { path, .. }) = data_type {
            // now handle options
            // first need to get actual type
            let data_type = syn::parse_str(
                &path.segments.first().and_then(|x| x.arguments.to_token_stream().into_iter().find_map(|x| 
                if let TokenTree::Ident(ident) = x {
                     Some(ident)
                } else {
                     None
                })).unwrap().to_string()
            ).unwrap();
            
            // generate read code for the underlying type
            let mut read = handle_simple_read(&data_type, endianness);
            // if conditional, update with required code
            if let Some(condition) = condition {
                read = generate_conditional_read(condition, read, &data_type);
            }
            // same for repetition
            if let Some(repetition) = repetition {
                read = generate_repeated_read(repetition, read);
            }

            quote! { let #id = #read? }
        } else {
            abort!(struct_name, "can only handle T, Vec<T> or Option<T>")
        }
    }).collect()
}

/// Generate the final structs with read implementations.
fn generate_structs(
    is_root: bool,
    root_name: &syn::Ident,
    struct_name: &syn::Ident,
    endianness: Endianness,
    items: &[Item],
) -> proc_macro2::TokenStream {
    // extract a list of types and ids from the item slice
    // needs to be two arrays because of how quote handles iterating
    let types: Vec<_> = items
        .iter()
        .map(|Item { data_type, repetition, condition, .. }| match (repetition, condition) {
            (Some(_), _) => 
                syn::parse_str(&format!("Vec<{}>", data_type.into_token_stream())).unwrap(),
            (None, Some(_)) => 
                syn::parse_str(&format!("Option<{}>", data_type.into_token_stream())).unwrap(),
            _ => quote! { #data_type }
        }).collect();
    let ids: Vec<_> = items.iter().map(|Item { id, .. }| quote! { #id}).collect();

    // then generate the list of read calls
    let read_calls = generate_read_calls(items, endianness, struct_name);

    // if is root, construct a struct context with all simple types before first complex type
    let context_name = format_ident!("{}Context", root_name);

    if is_root {
        /// Helper function to figure out if a type is "simple" - not a composite type
        fn is_simple_type(data_type: &proc_macro2::TokenStream) -> bool {
            if data_type.to_string().starts_with("Option") {
                // if its an option, check that the first identity (ignoring the "Option" itself) is a simple type
                is_simple_type(&data_type
                        .clone()
                        .into_iter()
                        .skip(1)
                        .find_map(|x| {
                            if let TokenTree::Ident(ident) = x {
                                Some(ident)
                            } else {
                                None
                            }
                        })
                        .unwrap()
                        .into_token_stream()
                )
            } else {
                // otherwise check if list of rust types contains it
                RUST_TYPES.contains(&data_type.to_string().as_str())
            }
        }

        // now take the first run of simple types/ids, needed to be able to generate the context struct at the correct point
        let simple_types: Vec<_> = types.iter()
            .take_while_ref(|t| is_simple_type(t))
            .collect();
        let simple_ids: Vec<_> = items
            .iter()
            .map(|Item { id, .. }| quote! { #id })
            .take(simple_types.len())
            .collect();

        // then split the read calls at the same point so context struct can be inserted in the middle
        let initial_read_calls = read_calls.iter().take(simple_types.len());
        let rest_read_calls = read_calls.iter().skip(simple_types.len());

        quote! {
            struct #context_name {
                #(pub #simple_ids: #simple_types),*
            }

            #[derive(Debug)]
            struct #struct_name {
                #(#ids: #types),*
            }

            impl #struct_name {
                pub fn read<R: ::byteorder::ReadBytesExt>(reader: &mut R) -> Option<Self> {
                    #(
                        #initial_read_calls;
                    )*

                    let _root = #context_name {
                        #(#simple_ids),*
                    };

                    #(
                        #rest_read_calls;
                    )*

                    Some(Self {
                        #(#ids),*
                    })
                }
            }
        }
    } else {
        quote! {
            #[derive(Debug)]
            struct #struct_name {
                #(#ids: #types),*
            }

            impl #struct_name {
                pub fn read<R: ::byteorder::ReadBytesExt>(reader: &mut R, _root: &#context_name) -> Option<Self> {
                    #(
                        #read_calls;
                    )*

                    Some(Self {
                        #(#ids),*
                    })
                }
            }
        }
    }
}

/// Generate the entire chunk of code to be inserted
pub(super) fn generate(struct_name: syn::Ident, format: Format) -> proc_macro::TokenStream {
    let types = format
        .types
        .iter()
        .map(|items| generate_structs(false, &struct_name, items.0, format.endianness, items.1));

    let main = generate_structs(
        true,
        &struct_name,
        &struct_name,
        format.endianness,
        &format.items,
    );

    quote! {
        #(#types)*
        #main
    }
    .into()
}
