use super::RUST_TYPES;
use crate::{
    generation::{statements::create_statement, Method},
    parse::Endianness,
    Condition, Item,
};
use proc_macro_error::abort;
use quote::{format_ident, quote, ToTokens};
use syn::{Type, TypePath};

/// Creates simple read code for the following 3 cases:
///     - Simple rust types like u16 where can just call reader function with correct endianness
///     - Booleans where need to do a simple conversion
///     - Composite types where we simply call the correct function
fn handle_simple_read(data_type: &syn::Type, endianness: Endianness) -> proc_macro2::TokenStream {
    // need to check if type is existing rust type or custom
    if RUST_TYPES.contains(&&*data_type.to_token_stream().to_string()) {
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

/// Generates a conditional read
pub(super) fn generate_conditional_read(
    condition: &Condition,
    statement: proc_macro2::TokenStream,
    data_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // make sure to advance pointer if needed
    let else_body = if condition.advance_if_false {
        quote! {
            reader.read_exact(&mut [0u8; std::mem::size_of::<#data_type>()]).ok()?;
            Some(None)
        }
    } else {
        quote! {
            Some(None)
        }
    };

    let expr = &condition.expression;
    quote! {
        if #expr {
            Some(#statement)
        } else {
            #else_body
        }
    }
}

/// Generates a vector of variable assignments that read the correct type from a reader.
pub(super) fn generate_read_calls(
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
                let read = handle_simple_read(data_type, endianness);
                let read = create_statement(read, id, data_type, condition, repetition, Method::Reading);

                quote! { let #id = #read? }
            } else {
                abort!(struct_name, "can only handle simple types (try removing any Options or Results in config file)")
            }
        })
        .collect()
}
