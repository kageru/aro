use time::Date;

use crate::{
    data::{BanlistStatus, Card},
    parser::{Field, Operator, RawCardFilter, Value},
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
    legal_copies:  i32,
    price:         Option<i32>,
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
                .filter_map(|s| SETS_BY_NAME.get(&s.set_name.to_lowercase()).and_then(|s| s.tcg_date))
                .map(Date::year)
                .min(),
            legal_copies:  card.banlist_info.map(|bi| bi.ban_tcg).unwrap_or(BanlistStatus::Unlimited) as i32,
            price:         card
                .card_prices
                .iter()
                .flat_map(|p| vec![p.cardmarket_price.parse::<f32>().ok(), p.tcgplayer_price.parse().ok()])
                .flatten()
                .map(|p| (p * 100.0) as i32)
                .min(),
        }
    }
}

pub type CardFilter = Box<dyn Fn(&SearchCard) -> bool>;

fn get_field_value(card: &SearchCard, field: Field) -> Option<Value> {
    Some(match field {
        Field::Atk => Value::Numerical(card.atk?),
        Field::Def => Value::Numerical(card.def?),
        Field::Legal => Value::Numerical(card.legal_copies),
        Field::Level => Value::Numerical(card.level?),
        Field::LinkRating => Value::Numerical(card.link_rating?),
        Field::Year => Value::Numerical(card.original_year?),
        Field::Set => Value::Multiple(card.sets.clone().into_iter().map(Value::String).collect()),
        Field::Type => Value::String(card.r#type.clone()),
        Field::Attribute => Value::String(card.attribute.clone().unwrap_or_default()),
        Field::Class => Value::String(card.card_type.clone()),
        Field::Name => Value::String(card.name.clone()),
        Field::Text => Value::String(card.text.clone()),
        Field::Price => Value::Numerical(card.price?),
    })
}

fn filter_value(op: &Operator, field_value: &Value, query_value: &Value) -> bool {
    match (field_value, query_value) {
        (Value::None, _) => false,
        (Value::Numerical(field), Value::Numerical(query)) => op.filter_number(Some(*field), *query),
        (Value::String(field), Value::String(query)) => match op {
            Operator::Equal => field.contains(query),
            Operator::NotEqual => !field.contains(query),
            // greater/less than aren’t supported for string fields.
            _ => false,
        },
        (Value::String(field), Value::Regex(query)) => match op {
            Operator::Equal => query.is_match(field),
            Operator::NotEqual => !query.is_match(field),
            // greater/less than aren’t supported for string fields.
            _ => false,
        },
        // Currently only for sets the card was released in.
        (Value::Multiple(field), query @ Value::String(_)) => match op {
            Operator::Equal => field.iter().any(|f| f == query),
            Operator::NotEqual => !field.iter().any(|f| f == query),
            _ => false,
        },
        _ => false,
    }
}

pub fn build_filter(RawCardFilter(field, op, value): RawCardFilter) -> Result<CardFilter, String> {
    Ok(match value {
        Value::Multiple(values) => Box::new(move |card: &SearchCard| {
            let field_value = get_field_value(card, field).unwrap_or_default();
            values.iter().any(|query_value| filter_value(&op, &field_value, query_value))
        }),
        single_value => Box::new(move |card: &SearchCard| {
            let field_value = get_field_value(card, field).unwrap_or_default();
            filter_value(&op, &field_value, &single_value)
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data::tests::{RAW_LINK_MONSTER, RAW_MONSTER},
        parser::parse_filters,
    };

    #[test]
    fn level_filter_test() {
        let lacooda = SearchCard::from(&serde_json::from_str::<Card>(RAW_MONSTER).unwrap());
        let lacooda_but_level_4 = SearchCard { level: Some(4), ..lacooda.clone() };

        let filter_level_3 = parse_filters("l=3").unwrap().1;
        assert!(filter_level_3[0](&lacooda));

        let filter_level_3_4 = parse_filters("l=3|4").unwrap().1;
        assert!(filter_level_3_4[0](&lacooda));
        assert!(filter_level_3_4[0](&lacooda_but_level_4));

        let filter_level_5 = parse_filters("l=5").unwrap().1;
        assert!(!filter_level_5[0](&lacooda));
    }

    #[test]
    fn filter_by_level_should_exclude_link_monsters() {
        let bls = SearchCard::from(&serde_json::from_str::<Card>(RAW_LINK_MONSTER).unwrap());
        let filter = parse_filters("l<=4").unwrap().1;
        assert!(!filter[0](&bls));
    }

    #[test]
    fn set_filter_test() {
        let lacooda = SearchCard::from(&serde_json::from_str::<Card>(RAW_MONSTER).unwrap());

        let astral_pack_filter = parse_filters("set:ap03").unwrap().1;
        assert!(astral_pack_filter[0](&lacooda));

        let partial_filter = parse_filters("set:ap0").unwrap().1;
        assert!(!partial_filter[0](&lacooda));

        let not_astral_pack_filter = parse_filters("set!=ap03").unwrap().1;
        assert!(!not_astral_pack_filter[0](&lacooda));

        let astral_pack_4_filter = parse_filters("set:ap04").unwrap().1;
        assert!(!astral_pack_4_filter[0](&lacooda));
    }

    #[test]
    fn regex_filter_test() {
        let lacooda = SearchCard::from(&serde_json::from_str::<Card>(RAW_MONSTER).unwrap());
        let bls = SearchCard::from(&serde_json::from_str::<Card>(RAW_LINK_MONSTER).unwrap());
        let draw_filter = parse_filters("o:/draw \\d cards?/").unwrap().1;
        assert!(draw_filter[0](&lacooda));
        assert!(!draw_filter[0](&bls));
    }

    #[test]
    fn price_filter_test() {
        let lacooda = SearchCard::from(&serde_json::from_str::<Card>(RAW_MONSTER).unwrap());
        let bls = SearchCard::from(&serde_json::from_str::<Card>(RAW_LINK_MONSTER).unwrap());
        let price_filter = parse_filters("p>300").unwrap().1;
        assert!(!price_filter[0](&lacooda));
        assert!(price_filter[0](&bls));
        let price_filter_2 = parse_filters("p<350").unwrap().1;
        assert!(price_filter_2[0](&bls), "Should filter by the cheaper version");
    }
}
