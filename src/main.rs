#![feature(option_result_contains)]
use nom::{
    branch::alt,
    bytes::complete::{take_until1, take_while, take_while_m_n},
    character::complete::{alphanumeric1, multispace0},
    combinator::{complete, map_res, rest},
    multi::many1,
    sequence::{preceded, tuple},
    IResult,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
};

type CardFilter = Box<dyn Fn(&SearchCard) -> bool>;
type RawCardFilter = (Field, Operator, Value);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cards = serde_json::from_reader::<_, CardInfo>(std::io::BufReader::new(std::fs::File::open("cards.json")?))?.data;
    let search_cards: Vec<_> = cards.iter().map(SearchCard::from).collect();
    let cards_by_id: HashMap<_, _> = cards.into_iter().map(|c| (c.id, c)).collect();
    let raw_query = std::env::args().nth(1).unwrap();
    let query = parse_filters(&raw_query)?;
    search_cards.iter().filter(|card| query.iter().all(|q| q(card))).for_each(|c| println!("{}", cards_by_id.get(&c.id).unwrap()));
    Ok(())
}

fn parse_filters(input: &str) -> Result<Vec<CardFilter>, String> {
    parse_raw_filters(input).map_err(|e| format!("Error while parsing filters “{input}”: {e:?}")).and_then(|(rest, v)| {
        if rest.is_empty() {
            Ok(v.into_iter().map(build_filter).collect::<Result<Vec<_>, _>>()?)
        } else {
            Err(format!("Input was not fully parsed. Left over: “{rest}”"))
        }
    })
}

fn parse_raw_filters(input: &str) -> IResult<&str, Vec<RawCardFilter>> {
    many1(parse_raw_filter)(input)
}

fn parse_raw_filter(input: &str) -> IResult<&str, RawCardFilter> {
    preceded(
        multispace0,
        alt((
            complete(tuple((field, operator, value))),
            map_res(take_until1(" "), |q| fallback_filter(q)),
            // I would like to use `rest` here, but that results in a pattern that can be empty
            // which can lead to infinite loops while parsing and is therefore disallowed by nom.
            // I would need something like rest1 or a way to assert that the rest isn’t empty.
            map_res(alphanumeric1, |q| fallback_filter(q)),
        )),
    )(input)
}

fn fallback_filter(query: &str) -> Result<RawCardFilter, String> {
    if query.contains(&OPERATOR_CHARS[..]) {
        return Err(format!("Invalid query: {query}"));
    }
    #[cfg(debug_assertions)]
    println!("Trying to match {query} as card name");
    let q = query.to_lowercase();
    Ok((Field::Name, Operator::Equals, Value::String(q)))
}

fn build_filter(query: RawCardFilter) -> Result<CardFilter, String> {
    dbg!(&query);
    Ok(match query {
        (Field::Atk, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.atk, n)),
        (Field::Def, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.def, n)),
        // ? ATK/DEF is modeled as None in the source json. At least for some monsters.
        // Let’s at least find those.
        (Field::Atk, _, Value::String(s)) if s == "?" => Box::new(move |card| card.atk.is_none() && card.card_type.contains("monster")),
        (Field::Def, _, Value::String(s)) if s == "?" => {
            Box::new(move |card| card.def.is_none() && card.link_rating.is_none() && card.card_type.contains("monster"))
        }
        (Field::Level, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.level, n)),
        (Field::Type, Operator::Equals, Value::String(s)) => Box::new(move |card| card.r#type == s),
        (Field::Class, Operator::Equals, Value::String(s)) => Box::new(move |card| card.card_type.contains(&s)),
        (Field::Text, Operator::Equals, Value::String(s)) => Box::new(move |card| card.text.contains(&s)),
        (Field::Name, Operator::Equals, Value::String(s)) => Box::new(move |card| card.name.contains(&s)),
        q => Err(format!("unknown query: {q:?}"))?,
    })
}

fn field(input: &str) -> IResult<&str, Field> {
    map_res(take_while(char::is_alphabetic), str::parse)(input)
}

const OPERATOR_CHARS: &[char] = &['=', '<', '>', ':'];

fn operator(input: &str) -> IResult<&str, Operator> {
    map_res(take_while_m_n(1, 2, |c| OPERATOR_CHARS.contains(&c)), str::parse)(input)
}

fn value(input: &str) -> IResult<&str, Value> {
    map_res(alt((take_until1(" "), rest)), |i: &str| match i.parse() {
        Ok(n) => Ok(Value::Numerical(n)),
        Err(_) if i.is_empty() => Err("empty filter argument"),
        Err(_) => Ok(Value::String(i.to_lowercase())),
    })(input)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Field {
    Atk,
    Def,
    Level,
    Type,
    Attribute,
    Class,
    Name,
    Text,
}

impl FromStr for Field {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_ref() {
            "atk" => Self::Atk,
            "def" => Self::Def,
            "level" | "l" => Self::Level,
            "type" | "t" => Self::Type,
            "attribute" | "attr" | "a" => Self::Attribute,
            "c" | "class" => Self::Class,
            "o" | "eff" | "text" | "effect" | "e" => Self::Text,
            _ => Err(s.to_string())?,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Operator {
    Equals,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl Operator {
    pub fn filter_number(&self, a: Option<i32>, b: i32) -> bool {
        if let Some(a) = a {
            match self {
                Self::Equals => a == b,
                Self::Less => a < b,
                Self::LessEqual => a <= b,
                Self::Greater => a > b,
                Self::GreaterEqual => a >= b,
            }
        } else {
            false
        }
    }
}

impl FromStr for Operator {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "=" | "==" | ":" => Self::Equals,
            ">=" | "=>" => Self::GreaterEqual,
            "<=" | "=<" => Self::LessEqual,
            ">" => Self::Greater,
            "<" => Self::Less,
            _ => Err(s.to_owned())?,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Value {
    String(String),
    Numerical(i32),
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
struct CardInfo {
    data: Vec<Card>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
struct Card {
    id:          usize,
    #[serde(rename = "type")]
    card_type:   String,
    name:        String,
    #[serde(rename = "desc")]
    text:        String,
    // Will also be None for ?
    atk:         Option<i32>,
    def:         Option<i32>,
    attribute:   Option<String>,
    #[serde(rename = "race")]
    r#type:      String,
    // also includes rank
    level:       Option<i32>,
    #[serde(rename = "linkval")]
    link_rating: Option<i32>,
    #[serde(rename = "linkmarkers")]
    link_arrows: Option<Vec<String>>,
}

/// A struct derived from `Card` that has all fields lowercased for easier search
#[derive(Debug, PartialEq, Eq, Clone)]
struct SearchCard {
    id:          usize,
    card_type:   String,
    name:        String,
    text:        String,
    atk:         Option<i32>,
    def:         Option<i32>,
    attribute:   Option<String>,
    r#type:      String,
    // also includes rank
    level:       Option<i32>,
    link_rating: Option<i32>,
    link_arrows: Option<Vec<String>>,
}

impl From<&Card> for SearchCard {
    fn from(card: &Card) -> Self {
        Self {
            id:          card.id,
            card_type:   card.card_type.to_lowercase(),
            name:        card.name.to_lowercase(),
            text:        card.text.to_lowercase(),
            atk:         card.atk,
            def:         card.def,
            attribute:   card.attribute.as_ref().map(|s| s.to_lowercase()),
            r#type:      card.r#type.to_lowercase(),
            level:       card.level,
            link_rating: card.link_rating,
            link_arrows: card.link_arrows.as_ref().map(|arrows| arrows.iter().map(|a| a.to_lowercase()).collect()),
        }
    }
}

impl Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (", &self.name)?;
        if let Some(level) = self.level {
            write!(f, "Level {level} ")?;
        } else if let Some(lr) = self.link_rating {
            write!(f, "Link {lr} ")?;
        }
        if let Some(attr) = &self.attribute {
            write!(f, "{attr}/")?;
        }
        write!(f, "{} {})", self.r#type, self.card_type)?;
        if self.card_type.contains(&String::from("Monster")) {
            match (self.atk, self.def) {
                (Some(atk), Some(def)) => write!(f, " {atk} ATK / {def} DEF")?,
                (Some(atk), None) if self.link_rating.is_some() => write!(f, "{atk} ATK")?,
                (None, Some(def)) => write!(f, " ? ATK / {def} DEF")?,
                (Some(atk), None) => write!(f, " {atk} ATK / ? DEF")?,
                (None, None) => write!(f, " ? ATK / ? DEF")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW_SPELL: &str = r#"
    {
      "id": 41142615,
      "name": "The Cheerful Coffin",
      "type": "Spell Card",
      "desc": "Discard up to 3 Monster Cards from your hand to the Graveyard.",
      "race": "Normal"
    }"#;

    const RAW_MONSTER: &str = r#"
    {
       "id": 2326738,
       "name": "Des Lacooda",
       "type": "Effect Monster",
       "desc": "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card.",
       "atk": 500,
       "def": 600,
       "level": 3,
       "race": "Zombie",
       "attribute": "EARTH"
    }"#;

    #[test]
    fn test_spell() {
        let coffin: Card = serde_json::from_str(RAW_SPELL).unwrap();
        assert_eq!(
            coffin,
            Card {
                id: 41142615,
                card_type: "Spell Card".to_owned(),
                name: "The Cheerful Coffin".to_owned(),
                text: "Discard up to 3 Monster Cards from your hand to the Graveyard.".to_owned(),
                r#type: "Normal".to_owned(),
                ..Default::default()
            }
        )
    }

    #[test]
    fn test_monster() {
        let munch: Card = serde_json::from_str(RAW_MONSTER).unwrap();
        assert_eq!(
            munch,
            Card {
                id: 2326738,
                card_type: "Effect Monster".to_owned(),
                name: "Des Lacooda".to_owned(),
                text:
                    "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card."
                        .to_owned(),
                atk: Some(500),
                def: Some(600),
                level: Some(3),
                r#type: "Zombie".to_owned(),
                attribute: Some("EARTH".to_owned()),
                ..Default::default()
            },
        )
    }

    #[test]
    fn query_parsing_test() {
        assert_eq!(parse_raw_filter("t:PYro"), Ok(("", (Field::Type, Operator::Equals, Value::String("pyro".into())))));
        assert_eq!(parse_raw_filter("t=pyro"), Ok(("", (Field::Type, Operator::Equals, Value::String("pyro".into())))));
        assert_eq!(parse_raw_filter("t==pyro"), Ok(("", (Field::Type, Operator::Equals, Value::String("pyro".into())))));
        assert_eq!(parse_raw_filter("atk>=100"), Ok(("", (Field::Atk, Operator::GreaterEqual, Value::Numerical(100)))));
        assert_eq!(parse_raw_filter("Necrovalley"), Ok(("", (Field::Name, Operator::Equals, Value::String("necrovalley".into())))));
        assert_eq!(parse_raw_filter("l=10"), Ok(("", (Field::Level, Operator::Equals, Value::Numerical(10)))));

        let (rest, filter) = parse_raw_filter("atk>=100 l:4").unwrap();
        assert_eq!(filter, (Field::Atk, Operator::GreaterEqual, Value::Numerical(100)));
        assert_eq!(parse_raw_filter(rest), Ok(("", (Field::Level, Operator::Equals, Value::Numerical(4)))));

        assert_eq!(
            parse_raw_filters("atk>=100 l:4"),
            Ok((
                "",
                vec![(Field::Atk, Operator::GreaterEqual, Value::Numerical(100)), (Field::Level, Operator::Equals, Value::Numerical(4))]
            ))
        );

        // These will fail during conversion
        assert!(parse_filters("l===10").is_err());
        assert!(parse_filters("t=").is_err());
        assert!(parse_filters("=100").is_err());
        assert!(parse_filters("atk<=>1").is_err());
    }

    #[test]
    fn level_filter_test() {
        let lacooda = SearchCard::from(&serde_json::from_str::<Card>(RAW_MONSTER).unwrap());
        let filter_level_3 = parse_filters("l=3").unwrap();
        assert!(filter_level_3[0](&lacooda));
        let filter_level_5 = parse_filters("l=5").unwrap();
        assert!(!filter_level_5[0](&lacooda));
    }
}
