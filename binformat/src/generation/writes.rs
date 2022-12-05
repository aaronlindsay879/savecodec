use super::RUST_TYPES;
use crate::{
    generation::{statements::create_statement, Method},
    parse::Endianness,
    Condition, Item,
};
use proc_macro_error::abort;
use quote::{format_ident, quote, ToTokens};
use syn::{Type, TypePath};

/// Creates simple write code for the following 3 cases:
///     - Simple rust types like u16 where can just call writer function with correct endianness
///     - Booleans where need to do a simple conversion
///     - Composite types where we simply call the correct function
fn handle_simple_write(
    id: &proc_macro2::TokenStream,
    data_type: &syn::Type,
    endianness: Endianness,
) -> proc_macro2::TokenStream {
    if RUST_TYPES.contains(&&*data_type.to_token_stream().to_string()) {
        // simple case where writer code exists, can just writer::write_<type>();

        let fn_call = format_ident!("write_{}", data_type.to_token_stream().to_string());

        match endianness {
            Endianness::Little => {
                quote! {  writer.#fn_call::<::byteorder::LittleEndian>(#id) }
            }
            Endianness::Big => {
                quote! { writer.#fn_call::<::byteorder::BigEndian>(#id) }
            }
        }
    } else if data_type.to_token_stream().to_string() == "bool" {
        // matches boolean logic in original savecodec2

        quote! { writer.write_u8(if #id { 1 } else { 0 }) }
    } else {
        quote! { #id.write(writer) }
    }
}

/// Generates a conditioanl write
pub(super) fn generate_conditional_write(
    condition: &Condition,
    id: &syn::Ident,
    statement: proc_macro2::TokenStream,
    data_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // advance pointer if needed, otherwies just return okay
    if condition.advance_if_false {
        quote! {
            if let Some(#id) = self.#id {
                #statement
            } else {
                writer.write_all(&[0u8; std::mem::size_of::<#data_type>()])
            }?
        }
    } else {
        quote! {
            if let Some(#id) = self.#id {
                #statement?
            }
        }
    }
}

/// Generates a vector of statements that write the correct type to a writer.
pub(super) fn generate_write_calls(
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
                let write =  if condition.is_some() || repetition.is_some() {
                    // if type has a condition or repetition, just pass the raw id and let the functions handle it
                    handle_simple_write(&quote! { #id }, data_type, endianness)
                } else {
                    // otherwise need to pass self.id
                    handle_simple_write(&quote! { self.#id }, data_type, endianness)
                };
                let write = create_statement(write, id, data_type, condition, repetition, Method::Writing);

                // conditional code has custom error handling, otherwise just standard error propagation
                if condition.is_some() {
                    quote! { #write }
                } else {
                    quote! { #write? }
                }
            } else {
                abort!(struct_name, "can only handle simple types (try removing any Options or Results in config file)")
            }
        })
        .collect()
}
