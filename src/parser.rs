use std::str::FromStr;

use crate::filter::{build_filter, fallback_filter, CardFilter, RawCardFilter};
use nom::{
    branch::alt,
    bytes::complete::{take_until1, take_while, take_while_m_n},
    character::complete::{char, multispace0},
    combinator::{complete, map_res, rest, verify},
    multi::many_m_n,
    sequence::{delimited, preceded, tuple},
    IResult,
};

pub fn parse_filters(input: &str) -> Result<Vec<CardFilter>, String> {
    parse_raw_filters(input).map_err(|e| format!("Error while parsing filters “{input}”: {e:?}")).and_then(|(rest, mut v)| {
        if rest.is_empty() {
            v.sort_unstable_by_key(|(f, _, _)| *f as u8);
            v.into_iter().map(build_filter).collect()
        } else {
            Err(format!("Input was not fully parsed. Left over: “{rest}”"))
        }
    })
}

fn parse_raw_filters(input: &str) -> IResult<&str, Vec<RawCardFilter>> {
    many_m_n(1, 32, parse_raw_filter)(input)
}

fn word_non_empty(input: &str) -> IResult<&str, &str> {
    verify(alt((take_until1(" "), rest)), |s: &str| s.len() >= 2)(input)
}

fn parse_raw_filter(input: &str) -> IResult<&str, RawCardFilter> {
    preceded(multispace0, alt((complete(tuple((field, operator, value))), map_res(word_non_empty, fallback_filter))))(input)
}

fn field(input: &str) -> IResult<&str, Field> {
    map_res(take_while(char::is_alphabetic), str::parse)(input)
}

pub const OPERATOR_CHARS: &[char] = &['=', '<', '>', ':', '!'];

fn operator(input: &str) -> IResult<&str, Operator> {
    map_res(take_while_m_n(1, 2, |c| OPERATOR_CHARS.contains(&c)), str::parse)(input)
}

fn value(input: &str) -> IResult<&str, Value> {
    map_res(alt((delimited(char('"'), take_until1("\""), char('"')), take_until1(" "), rest)), |i: &str| match i.parse() {
        Ok(n) => Ok(Value::Numerical(n)),
        Err(_) if i.is_empty() => Err("empty filter argument"),
        Err(_) => Ok(Value::String(i.to_lowercase())),
    })(input)
}

/// Ordinals are given highest = fastest to filter.
/// This is used to sort filters before applying them.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Field {
    Text = 0,
    Name = 1,
    Class = 2,
    Attribute = 3,
    Type = 4,
    Level = 5,
    Atk = 6,
    Def = 7,
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
pub enum Operator {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl Operator {
    pub fn filter_number(&self, a: Option<i32>, b: i32) -> bool {
        if let Some(a) = a {
            match self {
                Self::Equal => a == b,
                Self::Less => a < b,
                Self::LessEqual => a <= b,
                Self::Greater => a > b,
                Self::GreaterEqual => a >= b,
                Self::NotEqual => a != b,
            }
        } else {
            self == &Self::NotEqual
        }
    }
}

impl FromStr for Operator {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "=" | "==" | ":" => Self::Equal,
            ">=" | "=>" => Self::GreaterEqual,
            "<=" | "=<" => Self::LessEqual,
            ">" => Self::Greater,
            "<" => Self::Less,
            "!=" => Self::NotEqual,
            _ => Err(s.to_owned())?,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    String(String),
    Numerical(i32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("t=pyro" => Ok(("", (Field::Type, Operator::Equal, Value::String("pyro".into())))))]
    #[test_case("t:PYro" => Ok(("", (Field::Type, Operator::Equal, Value::String("pyro".into())))); "input is lowercased")]
    #[test_case("t==warrior" => Ok(("", (Field::Type, Operator::Equal, Value::String("warrior".into())))))]
    #[test_case("atk>=100" => Ok(("", (Field::Atk, Operator::GreaterEqual, Value::Numerical(100)))))]
    #[test_case("Necrovalley" => Ok(("", (Field::Name, Operator::Equal, Value::String("necrovalley".into())))))]
    #[test_case("l=10" => Ok(("", (Field::Level, Operator::Equal, Value::Numerical(10)))))]
    #[test_case("Ib" => Ok(("", (Field::Name, Operator::Equal, Value::String("ib".to_owned())))))]
    #[test_case("c!=synchro" => Ok(("", (Field::Class, Operator::NotEqual, Value::String("synchro".to_owned())))))]
    fn successful_parsing_test(input: &str) -> IResult<&str, RawCardFilter> {
        parse_raw_filter(input)
    }

    #[test_case("atk<=>1")]
    #[test_case("l===10")]
    #[test_case("t=")]
    #[test_case("=100")]
    #[test_case("a")]
    fn unsuccessful_parsing_test(input: &str) {
        assert!(parse_filters(input).is_err());
    }

    #[test]
    fn sequential_parsing_test() {
        let (rest, filter) = parse_raw_filter("atk>=100 l:4").unwrap();
        assert_eq!(filter, (Field::Atk, Operator::GreaterEqual, Value::Numerical(100)));
        assert_eq!(parse_raw_filter(rest), Ok(("", (Field::Level, Operator::Equal, Value::Numerical(4)))));

        assert_eq!(
            parse_raw_filters("atk>=100 l=4"),
            Ok((
                "",
                vec![(Field::Atk, Operator::GreaterEqual, Value::Numerical(100)), (Field::Level, Operator::Equal, Value::Numerical(4))]
            ))
        );

        assert_eq!(
            parse_raw_filters(r#"t:counter c:trap o:"negate the summon""#),
            Ok((
                "",
                vec![
                    (Field::Type, Operator::Equal, Value::String("counter".into())),
                    (Field::Class, Operator::Equal, Value::String("trap".into())),
                    (Field::Text, Operator::Equal, Value::String("negate the summon".into())),
                ]
            ))
        );
    }

    #[test]
    fn quoted_value_test() {
        let (rest, filter) = parse_raw_filter(r#"o:"destroy that target""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(filter, (Field::Text, Operator::Equal, Value::String("destroy that target".into())));
    }
}
