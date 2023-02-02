use time::Date;

use crate::{
    data::Card,
    parser::{Field, Operator, RawCardFilter, Value, OPERATOR_CHARS},
    SETS_BY_NAME,
};

/// A struct derived from `Card` that has all fields lowercased for easier search
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SearchCard {
    pub id:        usize,
    card_type:     String,
    name:          String,
    text:          String,
    atk:           Option<i32>,
    def:           Option<i32>,
    attribute:     Option<String>,
    r#type:        String,
    // also includes rank
    level:         Option<i32>,
    link_rating:   Option<i32>,
    link_arrows:   Option<Vec<String>>,
    sets:          Vec<String>,
    original_year: Option<i32>,
}

impl From<&Card> for SearchCard {
    fn from(card: &Card) -> Self {
        Self {
            id:            card.id,
            card_type:     card.card_type.to_lowercase(),
            name:          card.name.to_lowercase(),
            text:          card.text.to_lowercase(),
            atk:           card.atk,
            def:           card.def,
            attribute:     card.attribute.as_ref().map(|s| s.to_lowercase()),
            r#type:        card.r#type.to_lowercase(),
            level:         card.level,
            link_rating:   card.link_rating,
            link_arrows:   card.link_arrows.as_ref().map(|arrows| arrows.iter().map(|a| a.to_lowercase()).collect()),
            sets:          card.card_sets.iter().filter_map(|s| s.set_code.split('-').next().map(str::to_lowercase)).collect(),
            original_year: card
                .card_sets
                .iter()
                .filter_map(|s| {
                    SETS_BY_NAME.get(&s.set_name.to_lowercase()).unwrap_or_else(|| panic!("Set {} not found", s.set_name)).tcg_date
                })
                .map(Date::year)
                .min(),
        }
    }
}

pub type CardFilter = Box<dyn Fn(&SearchCard) -> bool>;

pub fn fallback_filter(query: &str) -> Result<RawCardFilter, String> {
    if query.contains(OPERATOR_CHARS) {
        return Err(format!("Invalid query: {query}"));
    }
    let q = query.to_lowercase();
    Ok(RawCardFilter(Field::Name, Operator::Equal, Value::String(q)))
}

pub fn build_filter(query: RawCardFilter) -> Result<CardFilter, String> {
    Ok(match query {
        RawCardFilter(Field::Atk, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.atk, n)),
        RawCardFilter(Field::Def, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.def, n)),
        // ? ATK/DEF is modeled as None in the source json. At least for some monsters.
        // Let’s at least find those.
        RawCardFilter(Field::Atk, _, Value::String(s)) if s == "?" => {
            Box::new(move |card| card.atk.is_none() && card.card_type.contains("monster"))
        }
        RawCardFilter(Field::Def, _, Value::String(s)) if s == "?" => {
            Box::new(move |card| card.def.is_none() && card.link_rating.is_none() && card.card_type.contains("monster"))
        }
        RawCardFilter(Field::Level, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.level, n)),
        RawCardFilter(Field::LinkRating, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.link_rating, n)),
        RawCardFilter(Field::Year, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.original_year, n)),
        RawCardFilter(Field::Type, Operator::Equal, Value::String(s)) => Box::new(move |card| card.r#type == s),
        RawCardFilter(Field::Type, Operator::NotEqual, Value::String(s)) => Box::new(move |card| card.r#type != s),
        RawCardFilter(Field::Attribute, Operator::Equal, Value::String(s)) => Box::new(move |card| card.attribute.contains(&s)),
        RawCardFilter(Field::Attribute, Operator::NotEqual, Value::String(s)) => Box::new(move |card| !card.attribute.contains(&s)),
        RawCardFilter(Field::Class, Operator::Equal, Value::String(s)) => Box::new(move |card| card.card_type.contains(&s)),
        RawCardFilter(Field::Class, Operator::NotEqual, Value::String(s)) => Box::new(move |card| !card.card_type.contains(&s)),
        RawCardFilter(Field::Text, Operator::Equal, Value::String(s)) => Box::new(move |card| card.text.contains(&s)),
        RawCardFilter(Field::Text, Operator::NotEqual, Value::String(s)) => Box::new(move |card| !card.text.contains(&s)),
        RawCardFilter(Field::Name, Operator::Equal, Value::String(s)) => Box::new(move |card| card.name.contains(&s)),
        RawCardFilter(Field::Name, Operator::NotEqual, Value::String(s)) => Box::new(move |card| !card.name.contains(&s)),
        RawCardFilter(Field::Set, Operator::Equal, Value::String(s)) => Box::new(move |card| card.sets.contains(&s)),
        q => Err(format!("unknown query: {q:?}"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{data::tests::RAW_MONSTER, parser::parse_filters};

    #[test]
    fn level_filter_test() {
        let lacooda = SearchCard::from(&serde_json::from_str::<Card>(RAW_MONSTER).unwrap());
        let filter_level_3 = parse_filters("l=3").unwrap();
        assert!(filter_level_3.1[0](&lacooda));
        let filter_level_5 = parse_filters("l=5").unwrap();
        assert!(!filter_level_5.1[0](&lacooda));
    }
}
