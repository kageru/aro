#![feature(option_result_contains)]
use nom::{
    bytes::{complete::take_while_m_n, streaming::take_while},
    combinator::{map_res, rest},
    sequence::tuple,
    IResult,
};
use serde::{de::Visitor, Deserialize, Deserializer};
use std::{
    fmt::{self, Display},
    str::FromStr,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cards = serde_json::from_reader::<_, CardInfo>(std::io::BufReader::new(std::fs::File::open("cards.json")?))?.data;
    let query = std::env::args()
        .skip(1)
        .map(|q| {
            query_arg(&q).map(|(_, r)| build_filter(r)).unwrap_or_else(|_| {
                println!("Trying to match {} as card name", q);
                let q = q.to_lowercase();
                Box::new(move |card: &Card| card.name.to_lowercase().contains(&q))
            })
        })
        .collect::<Vec<Box<dyn Fn(&Card) -> bool>>>();

    cards.iter().filter(|card| query.iter().all(|q| q(card))).for_each(|c| println!("{c}"));

    Ok(())
}

fn query_arg(input: &str) -> IResult<&str, (Field, Operator, Value)> {
    tuple((field, operator, value))(input)
}

fn build_filter(query: (Field, Operator, Value)) -> Box<dyn Fn(&Card) -> bool> {
    // dbg!("Building filter for {query:?}");
    match query {
        (Field::Atk, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.atk, n)),
        (Field::Def, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.def, n)),
        (Field::Level, op, Value::Numerical(n)) => Box::new(move |card| op.filter_number(card.level, n)),
        (Field::Type, Operator::Equals, Value::String(s)) => Box::new(move |card| card.r#type.to_lowercase() == s.to_lowercase()),
        (Field::Attribute, Operator::Equals, Value::String(s)) => {
            Box::new(move |card| card.attribute.as_ref().map(|s| s.to_lowercase()).contains(&s.to_lowercase()))
        }
        (Field::Class, Operator::Equals, Value::String(s)) => {
            let s = s.to_lowercase();
            Box::new(move |card| card.card_type.iter().map(|t| t.to_lowercase()).any(|t| t == s))
        }
        q => {
            println!("unknown query: {q:?}");
            Box::new(|_| false)
        }
    }
}

fn field(input: &str) -> IResult<&str, Field> {
    map_res(take_while(char::is_alphabetic), str::parse)(input)
}

const OPERATOR_CHARS: &[char] = &['=', '<', '>', ':'];

fn operator(input: &str) -> IResult<&str, Operator> {
    map_res(take_while_m_n(1, 2, |c| OPERATOR_CHARS.contains(&c)), str::parse)(input)
}

fn value(input: &str) -> IResult<&str, Value> {
    map_res(rest, |i: &str| match i.parse() {
        Ok(n) => Result::<_, ()>::Ok(Value::Numerical(n)),
        Err(_) => Ok(Value::String(i.to_owned())),
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

fn split_types<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    struct SplittingVisitor;

    impl<'de> Visitor<'de> for SplittingVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.split_whitespace().filter(|t| t != &"Card").map(str::to_owned).collect())
        }
    }
    deserializer.deserialize_any(SplittingVisitor)
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
#[serde(tag = "type")]
struct Card {
    #[serde(rename = "type", deserialize_with = "split_types")]
    card_type:   Vec<String>,
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
    linkmarkers: Option<Vec<String>>,
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
        f.write_str(&self.r#type)?;
        write!(f, " {})", self.card_type.join(" "))?;
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

    #[test]
    fn test_spell() {
        let s = r#"
        {
          "id": 41142615,
          "name": "The Cheerful Coffin",
          "type": "Spell Card",
          "desc": "Discard up to 3 Monster Cards from your hand to the Graveyard.",
          "race": "Normal"
        }"#;
        let coffin: Card = serde_json::from_str(s).unwrap();
        assert_eq!(
            coffin,
            Card {
                card_type: vec!["Spell".to_owned()],
                name: "The Cheerful Coffin".to_owned(),
                text: "Discard up to 3 Monster Cards from your hand to the Graveyard.".to_owned(),
                r#type: "Normal".to_owned(),
                ..Default::default()
            }
        )
    }

    #[test]
    fn test_monster() {
        let s = r#"
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
        let munch: Card = serde_json::from_str(s).unwrap();
        assert_eq!(
            munch,
            Card {
                card_type: vec!["Effect".to_owned(), "Monster".to_owned()],
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
}
