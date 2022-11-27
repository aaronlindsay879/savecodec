use crate::{Condition, Repetition};
use proc_macro2::TokenStream;
use quote::quote;

use super::{reads::generate_conditional_read, writes::generate_conditional_write, Method};

/// Generates a conditional statement from the arguments given.
fn generate_conditional_statement(
    condition: &Condition,
    id: &syn::Ident,
    statement: proc_macro2::TokenStream,
    data_type: &syn::Type,
    method: Method,
) -> proc_macro2::TokenStream {
    match method {
        Method::Reading => generate_conditional_read(condition, statement, data_type),
        Method::Writing => generate_conditional_write(condition, id, statement, data_type),
    }
}

/// Generates a repeated statement from the arguments given.
fn generate_repeated_statement(
    repetition: &Repetition,
    id: &syn::Ident,
    statement: proc_macro2::TokenStream,
    method: Method,
) -> proc_macro2::TokenStream {
    match repetition {
        Repetition::Count(expr) => match method {
            Method::Reading => quote! {
                (0..#expr).map(|_| #statement).collect::<Option<Vec<_>>>()
            },
            Method::Writing => quote! {
                self.#id
                    .iter()
                    .map(|#id| #statement)
                    .collect::<Option<Vec<_>>>()
            },
        },
    }
}

/// Creates a final statement with all required conditional and repetition code
pub(super) fn create_statement(
    mut original: TokenStream,
    id: &syn::Ident,
    data_type: &syn::Type,
    condition: &Option<Condition>,
    repetition: &Option<Repetition>,
    method: Method,
) -> proc_macro2::TokenStream {
    // if conditional, update with required code
    if let Some(condition) = condition {
        original = generate_conditional_statement(condition, id, original, data_type, method);
    }
    // same for repetition
    if let Some(repetition) = repetition {
        original = generate_repeated_statement(repetition, id, original, method);
    }

    original
}
