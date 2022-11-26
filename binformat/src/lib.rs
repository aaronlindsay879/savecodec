#![feature(let_chains)]

mod generate;
mod parse;

use crate::parse::parse_file;
use parse::Endianness;
use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use serde_yaml::Value;
use std::collections::{BTreeMap, HashMap};
use syn::{parse_macro_input, AttributeArgs, ItemStruct, Lit};

#[derive(Debug, Clone)]
struct Item {
    id: syn::Ident,
    data_type: syn::Type,
    condition: Option<syn::ExprBinary>,
}

#[derive(Debug)]
struct Format {
    endianness: Endianness,
    types: HashMap<syn::Ident, Vec<Item>>,
    items: Vec<Item>,
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn format_source(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let item = parse_macro_input!(item as ItemStruct);

    let path = if let [syn::NestedMeta::Lit(Lit::Str(path))] = &args[..] {
        path.value()
    } else {
        abort!(
            item.attrs.first(),
            "Expected a string literal for the path."
        )
    };

    let struct_name = item.ident;

    let file_contents = std::fs::read_to_string(path)
        .unwrap_or_else(|_| abort!(item.attrs.first(), "Path provided is not a valid file."));
    let file: BTreeMap<String, Value> = serde_yaml::from_str(&file_contents)
        .unwrap_or_else(|_| abort!(item.attrs.first(), "Path provided is not valid yaml."));

    let format = parse_file(file)
        .unwrap_or_else(|| abort!(item.attrs.first(), "File provided is not a valid format."));

    generate::generate(struct_name, format)
}
