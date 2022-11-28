use crate::{Condition, Format, Item, Repetition};
use serde_yaml::{Mapping, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Endianness {
    Little,
    Big,
}

/// Parses the meta entry to find the endianness, defaulting to little endian
fn parse_meta(meta: Option<&Value>) -> Endianness {
    let is_be = meta
        .and_then(|val| val.get("endian"))
        .map_or(false, |endianness| endianness.as_str() == Some("be"));

    if is_be {
        Endianness::Big
    } else {
        Endianness::Little
    }
}

fn parse_repetition(value: &str) -> Option<Repetition> {
    let mut chars = value.chars();

    let discriminant = chars.by_ref().take_while(|&c| c != '(').collect::<String>();
    let expression = chars.by_ref().take_while(|&c| c != ')').collect::<String>();

    match &discriminant[..] {
        "Count" => Some(Repetition::Count(syn::parse_str(&expression).ok()?)),
        _ => None,
    }
}

/// Parse an individual item
fn parse_item(item: &Mapping) -> Option<Item> {
    let id = syn::parse_str(item.get("id")?.as_str()?).ok()?;
    let data_type = syn::parse_str(item.get("type")?.as_str()?).ok()?;
    let condition_expr = item
        .get("if")
        .and_then(Value::as_str)
        .and_then(|cond| syn::parse_str(cond).ok());
    let repetition = item
        .get("repeat")
        .and_then(Value::as_str)
        .and_then(parse_repetition);
    let advance_if_false = item
        .get("advance_if_false")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let condition = condition_expr.map(|expression| Condition {
        expression,
        advance_if_false,
    });

    Some(Item {
        id,
        data_type,
        condition,
        repetition,
    })
}

/// Parse a sequence of values
fn parse_sequence(item: Option<&Value>) -> Vec<Item> {
    item.and_then(|val| val.as_sequence())
        .map_or_else(Vec::new, |val| {
            val.iter()
                .filter_map(|value| value.as_mapping().and_then(parse_item))
                .collect()
        })
}

/// Parse the user-defined types
fn parse_defined_types(item: Option<&Value>) -> HashMap<syn::Ident, Vec<Item>> {
    fn parse_defined_type((name, items): (&Value, &Value)) -> Option<(syn::Ident, Vec<Item>)> {
        let type_name = syn::parse_str(name.as_str()?).ok()?;
        let items = parse_sequence(Some(items));

        Some((type_name, items))
    }

    item.and_then(|val| val.as_mapping())
        .map_or_else(HashMap::new, |val| {
            val.iter().filter_map(parse_defined_type).collect()
        })
}

/// Parse the entire file, returning a format if it is valid
pub(super) fn parse_file(items: BTreeMap<String, Value>) -> Option<Format> {
    let endianness = parse_meta(items.get("meta"));
    let types = parse_defined_types(items.get("types"));
    let items = parse_sequence(items.get("items"));

    Some(Format {
        endianness,
        types,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::{Mapping, Value};

    #[test]
    fn parse_meta_test() {
        assert_eq!(parse_meta(None), Endianness::Little);

        let le_value = {
            let mut le_value = Mapping::new();
            le_value.insert(
                Value::String("endian".to_owned()),
                Value::String("le".to_owned()),
            );
            Value::Mapping(le_value)
        };
        assert_eq!(parse_meta(Some(&le_value)), Endianness::Little);

        let be_value = {
            let mut be_value = Mapping::new();
            be_value.insert(
                Value::String("endian".to_owned()),
                Value::String("be".to_owned()),
            );
            Value::Mapping(be_value)
        };
        assert_eq!(parse_meta(Some(&be_value)), Endianness::Big);

        let other_value = {
            let mut other_value = Mapping::new();
            other_value.insert(
                Value::String("endian".to_owned()),
                Value::String("other".to_owned()),
            );
            Value::Mapping(other_value)
        };
        assert_eq!(parse_meta(Some(&other_value)), Endianness::Little);
    }
}
