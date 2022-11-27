use crate::{parse::Endianness, Item};

use super::{reads::generate_read_calls, writes::generate_write_calls, RUST_TYPES};
use itertools::Itertools;
use quote::{format_ident, quote, ToTokens};

/// Generates the root struct and assosciated context
fn generate_root_struct(
    struct_name: &syn::Ident,
    types: Vec<proc_macro2::TokenStream>,
    ids: Vec<proc_macro2::TokenStream>,
    read_calls: Vec<proc_macro2::TokenStream>,
    write_calls: Vec<proc_macro2::TokenStream>,
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

        #[derive(Debug, PartialEq)]
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

            pub fn write<W: ::byteorder::WriteBytesExt>(&self, writer: &mut W) -> Option<()> {
                #(
                    #write_calls;
                )*

                Some(())
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
    write_calls: Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let context_name = format_ident!("{}Context", root_name);

    quote! {
        #[derive(Debug, PartialEq)]
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

            pub fn write<W: ::byteorder::WriteBytesExt>(&self, writer: &mut W) -> Option<()> {
                #(
                    #write_calls;
                )*

                Some(())
            }
        }
    }
}

/// Generate a struct with given information with read implementation, correctly handling the root case.
pub(super) fn generate_struct(
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

    // then generate the list of calls
    let read_calls = generate_read_calls(items, endianness, struct_name);
    let write_calls = generate_write_calls(items, endianness, struct_name);

    // simple check for root struct
    if struct_name == root_name {
        generate_root_struct(struct_name, types, ids, read_calls, write_calls)
    } else {
        generate_composite_struct(struct_name, root_name, types, ids, read_calls, write_calls)
    }
}
