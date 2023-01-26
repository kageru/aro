use std::str::FromStr;

use crate::filter::{build_filter, fallback_filter, CardFilter, RawCardFilter};
use nom::{
    branch::alt,
    bytes::complete::{take_until1, take_while, take_while_m_n},
    character::complete::multispace0,
    combinator::{complete, map_res, rest, verify},
    multi::many_m_n,
    sequence::{preceded, tuple},
    IResult,
};

pub fn parse_filters(input: &str) -> Result<Vec<CardFilter>, String> {
    parse_raw_filters(input).map_err(|e| format!("Error while parsing filters “{input}”: {e:?}")).and_then(|(rest, v)| {
        if rest.is_empty() {
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

pub const OPERATOR_CHARS: &[char] = &['=', '<', '>', ':'];

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
pub enum Field {
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
pub enum Operator {
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
pub enum Value {
    String(String),
    Numerical(i32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("t=pyro" => Ok(("", (Field::Type, Operator::Equals, Value::String("pyro".into())))))]
    #[test_case("t:PYro" => Ok(("", (Field::Type, Operator::Equals, Value::String("pyro".into())))); "input is lowercased")]
    #[test_case("t==warrior" => Ok(("", (Field::Type, Operator::Equals, Value::String("warrior".into())))))]
    #[test_case("atk>=100" => Ok(("", (Field::Atk, Operator::GreaterEqual, Value::Numerical(100)))))]
    #[test_case("Necrovalley" => Ok(("", (Field::Name, Operator::Equals, Value::String("necrovalley".into())))))]
    #[test_case("l=10" => Ok(("", (Field::Level, Operator::Equals, Value::Numerical(10)))))]
    #[test_case("Ib" => Ok(("", (Field::Name, Operator::Equals, Value::String("ib".to_owned())))))]
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
        assert_eq!(parse_raw_filter(rest), Ok(("", (Field::Level, Operator::Equals, Value::Numerical(4)))));

        assert_eq!(
            parse_raw_filters("atk>=100 l:4"),
            Ok((
                "",
                vec![(Field::Atk, Operator::GreaterEqual, Value::Numerical(100)), (Field::Level, Operator::Equals, Value::Numerical(4))]
            ))
        );
    }
}
