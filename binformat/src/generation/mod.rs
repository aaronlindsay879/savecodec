mod reads;
mod statements;
mod structs;
mod writes;

use crate::Format;
use quote::quote;
use structs::generate_struct;

#[derive(Clone, Copy)]
enum Method {
    Reading,
    Writing,
}

const RUST_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64",
];

/// Generate the entire chunk of code to be inserted
pub(super) fn generate(item: syn::ItemStruct, format: Format) -> proc_macro::TokenStream {
    let types = format
        .types
        .iter()
        .map(|items| generate_struct(&item, items.0, format.endianness, items.1));

    let main = generate_struct(&item, &item.ident, format.endianness, &format.items);

    quote! {
        #(#types)*
        #main
    }
    .into()
}
