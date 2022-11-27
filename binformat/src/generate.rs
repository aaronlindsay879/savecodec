use crate::{parse::Endianness, Condition, Format, Item, Repetition};
use itertools::Itertools;
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
        // matches boolean logic in original savecodec2

        quote! { reader.read_u8().map(|i| i != 0).ok() }
    } else {
        // more complex case where needs to use custom implementation
        // pass root context for conditional support
        // e.g. <type>::read(&reader, &_root);

        quote! { #data_type::read(reader, &_root) }
    }
}

/// Generates a conditional read from the arguments given.
fn generate_conditional_read(
    condition: &Condition,
    read: proc_macro2::TokenStream,
    data_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // make sure to advance read pointer if needed
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
fn generate_repeated_read(
    repetition: &Repetition,
    read: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match repetition {
        Repetition::Count(expr) => {
            quote! {
                (0..#expr).map(|_| #read).collect::<Option<Vec<_>>>()
            }
        }
    }
}

/// Creates a final read statement with all required conditional and repetition code
fn create_read(
    data_type: &syn::Type,
    endianness: Endianness,
    condition: &Option<Condition>,
    repetition: &Option<Repetition>,
) -> proc_macro2::TokenStream {
    let mut original = handle_simple_read(data_type, endianness);

    // if conditional, update with required code
    if let Some(condition) = condition {
        original = generate_conditional_read(condition, original, data_type);
    }
    // same for repetition
    if let Some(repetition) = repetition {
        original = generate_repeated_read(repetition, original);
    }

    original
}

/// Generates a vector of variable assignments that read the correct type from a reader.
fn generate_read_calls(
    items: &[Item],
    endianness: Endianness,
    struct_name: &syn::Ident,
) -> Vec<proc_macro2::TokenStream> {
    /// Checks if type contains any symbols which indicate if it's a complex type (like `Option<T>`)
    #[inline(always)]
    fn is_simple_type(path: &syn::Path) -> bool {
        path.segments
            .first()
            .map(|x| !x.ident.to_string().contains("<>"))
            .unwrap_or(false)
    }

    items
        .iter()
        .map(|item| {
            let Item {
                id,
                data_type,
                condition,
                repetition,
            } = item;

            if let Type::Path(TypePath { path, .. }) = data_type && is_simple_type(path) {
                let read = create_read(data_type, endianness, condition, repetition);

                quote! { let #id = #read? }
            } else {
                abort!(struct_name, "can only handle simple types (try removing any Options or Results in config file)")
            }
        })
        .collect()
}

/// Generates the root struct and assosciated context
fn generate_root_struct(
    struct_name: &syn::Ident,
    types: Vec<proc_macro2::TokenStream>,
    ids: Vec<proc_macro2::TokenStream>,
    read_calls: Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    // if is root, construct a struct context with all simple types before first complex type
    let context_name = format_ident!("{}Context", struct_name);

    /// Helper function to figure out if a type is "simple" - not a composite type
    fn is_simple_type(data_type: &proc_macro2::TokenStream) -> bool {
        // check if list of rust types contains it
        RUST_TYPES.contains(&data_type.to_string().as_str())
    }

    // now take the first run of simple types/ids, needed to be able to generate the context struct at the correct point
    let simple_types: Vec<_> = types.iter().take_while_ref(|t| is_simple_type(t)).collect();
    let simple_ids: Vec<_> = ids.iter().take(simple_types.len()).collect();

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
}

/// Generates a composite struct for user defined types
fn generate_composite_struct(
    struct_name: &syn::Ident,
    root_name: &syn::Ident,
    types: Vec<proc_macro2::TokenStream>,
    ids: Vec<proc_macro2::TokenStream>,
    read_calls: Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let context_name = format_ident!("{}Context", root_name);

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

/// Generate a struct with given information with read implementation, correctly handling the root case.
fn generate_struct(
    root_name: &syn::Ident,
    struct_name: &syn::Ident,
    endianness: Endianness,
    items: &[Item],
) -> proc_macro2::TokenStream {
    // extract a list of types and ids from the item slice
    // needs to be two arrays because of how quote handles iterating
    let types: Vec<_> = items
        .iter()
        .map(
            |Item {
                 data_type,
                 repetition,
                 condition,
                 ..
             }| match (repetition, condition) {
                (Some(_), _) => {
                    syn::parse_str(&format!("Vec<{}>", data_type.into_token_stream())).unwrap()
                }
                (None, Some(_)) => {
                    syn::parse_str(&format!("Option<{}>", data_type.into_token_stream())).unwrap()
                }
                _ => quote! { #data_type },
            },
        )
        .collect();
    let ids: Vec<_> = items.iter().map(|Item { id, .. }| quote! { #id}).collect();

    // then generate the list of read calls
    let read_calls = generate_read_calls(items, endianness, struct_name);

    // simple check for root struct
    if struct_name == root_name {
        generate_root_struct(struct_name, types, ids, read_calls)
    } else {
        generate_composite_struct(struct_name, root_name, types, ids, read_calls)
    }
}

/// Generate the entire chunk of code to be inserted
pub(super) fn generate(struct_name: syn::Ident, format: Format) -> proc_macro::TokenStream {
    let types = format
        .types
        .iter()
        .map(|items| generate_struct(&struct_name, items.0, format.endianness, items.1));

    let main = generate_struct(&struct_name, &struct_name, format.endianness, &format.items);

    quote! {
        #(#types)*
        #main
    }
    .into()
}
