use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crate::filter::{CardFilter, build_filter};
use itertools::Itertools;
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{take_until1, take_while, take_while_m_n},
    character::complete::{char, multispace0},
    combinator::{complete, map, map_res, recognize, rest, verify},
    multi::{many_m_n, separated_list1},
    sequence::{delimited, preceded, tuple},
};
use regex::Regex;

pub fn parse_filters(input: &str) -> Result<(Vec<RawCardFilter>, Vec<CardFilter>), String> {
    parse_raw_filters(input).map_err(|e| format!("Error while parsing filters “{input}”: {e:?}")).and_then(|(rest, mut v)| {
        if rest.is_empty() {
            // Sorting must be stable or we can’t combine multiple name filters into one.
            v.sort_by_key(|RawCardFilter(f, _, _)| *f as u8);
            // Combine multiple names searches into one search filter. This makes the readable query nicer
            // (“Showing 21 results where name is ally and name is of and name is justice” becomes
            // “Showing 21 results where name is ‘ally of justice’”)
            // and improves search performance by only performing one String::contains.
            // This could be done without allocating two vectors, but coalesce is just so much nicer.
            v = v
                .into_iter()
                .coalesce(|a, b| match (&a, &b) {
                    (
                        RawCardFilter(Field::Name, Operator::Equal, Value::String(s1)),
                        RawCardFilter(Field::Name, Operator::Equal, Value::String(s2)),
                    ) => Ok(RawCardFilter(Field::Name, Operator::Equal, Value::String(format!("{s1} {s2}")))),
                    _ => Err((a, b)),
                })
                .collect();
            Ok((v.clone(), v.clone().into_iter().map(|r| build_filter(r)).collect::<Result<Vec<_>, _>>()?))
        } else {
            Err(format!("Input was not fully parsed. Left over: “{rest}”"))
        }
    })
}

fn parse_raw_filters(input: &str) -> IResult<&str, Vec<RawCardFilter>> {
    many_m_n(1, 32, parse_raw_filter)(input)
}

fn word_non_empty(input: &str) -> IResult<&str, &str> {
    verify(alt((take_until1(" "), rest)), |s: &str| !s.is_empty())(input)
}

fn sanitize(query: &str) -> Result<String, String> {
    if query.is_empty() { Err("Query must not be empty".to_owned()) } else { Ok(query.to_lowercase()) }
}

fn fallback_filter(query: &str) -> Result<RawCardFilter, String> {
    Ok(RawCardFilter(Field::Name, Operator::Equal, Value::String(sanitize(query)?)))
}

fn parse_raw_filter(input: &str) -> IResult<&str, RawCardFilter> {
    preceded(
        multispace0,
        alt((
            map(complete(tuple((field, operator, values))), |(f, o, v)| RawCardFilter(f, o, v)),
            map_res(word_non_empty, fallback_filter),
        )),
    )(input)
}

fn field(input: &str) -> IResult<&str, Field> {
    map_res(take_while(char::is_alphabetic), str::parse)(input)
}

pub const OPERATOR_CHARS: &[char] = &['=', '<', '>', ':', '!'];

fn operator(input: &str) -> IResult<&str, Operator> {
    map_res(take_while_m_n(1, 2, |c| OPERATOR_CHARS.contains(&c)), str::parse)(input)
}

fn values(input: &str) -> IResult<&str, Value> {
    map_res(
        alt((
            delimited(char('"'), take_until1("\""), char('"')),
            recognize(delimited(char('/'), take_until1("/"), char('/'))),
            recognize(separated_list1(char('|'), take_until1(" |"))),
            take_until1(" "),
            rest,
        )),
        parse_values,
    )(input)
}

fn parse_values(input: &str) -> Result<Value, String> {
    Ok(if let Some(regex) = input.strip_prefix('/').and_then(|i| i.strip_suffix('/')) {
        Value::Regex(Regex::new(&regex.to_lowercase()).map_err(|_| format!("Invalid regex: {regex}"))?)
    } else {
        let values = input.split('|').map(parse_single_value).collect::<Result<Vec<Value>, String>>()?;
        match values.as_slice() {
            [v] => v.clone(),
            _ => Value::Multiple(values),
        }
    })
}

fn parse_single_value(input: &str) -> Result<Value, String> {
    Ok(match input.parse() {
        Ok(n) => Value::Numerical(n),
        Err(_) => Value::String(sanitize(input)?),
    })
}

/// Ordinals are given highest = fastest to filter.
/// This is used to sort filters before applying them.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Field {
    Atk = 1,
    Def = 2,
    Legal = 3,
    Level = 4,
    Genesys = 5,
    LinkRating = 6,
    Year = 8,
    Price = 9,
    Set = 10,
    Type = 12,
    Attribute = 14,
    Name = 18,
    Text = 20,
}

impl Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "text",
            Self::Name => "name",
            Self::Attribute => "attribute",
            Self::Type => "type",
            Self::Level => "level/rank",
            Self::Atk => "ATK",
            Self::Def => "DEF",
            Self::LinkRating => "link rating",
            Self::Genesys => "genesys points",
            Self::Set => "set",
            Self::Year => "year",
            Self::Legal => "allowed copies",
            Self::Price => "price",
        })
    }
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
            "o" | "eff" | "text" | "effect" | "e" => Self::Text,
            "lr" | "linkrating" => Self::LinkRating,
            "g" | "gen" | "genesys" => Self::Genesys,
            "name" => Self::Name,
            "set" | "s" => Self::Set,
            "year" | "y" => Self::Year,
            "legal" | "copies" => Self::Legal,
            "price" | "p" => Self::Price,
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
    pub fn filter_number(&self, a: i32, b: i32) -> bool {
        match self {
            Self::Equal => a == b,
            Self::Less => a < b,
            Self::LessEqual => a <= b,
            Self::Greater => a > b,
            Self::GreaterEqual => a >= b,
            Self::NotEqual => a != b,
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

impl Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Equal => "is",
            Self::NotEqual => "is not",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RawCardFilter(pub Field, pub Operator, pub Value);

impl Display for RawCardFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, Default)]
pub enum Value {
    String(String),
    Regex(Regex),
    Numerical(i32),
    // Multiple values that should only match exactly, e.g. set codes.
    Multiple(Vec<Value>),
    // Multiple values that should support partial matching, e.g. Card Name (YGOrg translation + official).
    MultiplePartial(Vec<String>),
    #[default]
    None,
}

// Manually implementing this because `Regex` isn’t `PartialEq`
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(s1), Value::String(s2)) => s1 == s2,
            (Value::Numerical(a), Value::Numerical(b)) => a == b,
            (Value::Multiple(v1), Value::Multiple(v2)) => v1 == v2,
            (Value::MultiplePartial(v1), Value::MultiplePartial(v2)) => v1 == v2,
            (Value::Regex(r1), Value::Regex(r2)) => r1.as_str() == r2.as_str(),
            (Value::None, Value::None) => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::String(s) => {
                if s.contains(' ') {
                    write!(f, "\"{s}\"")
                } else {
                    f.write_str(s)
                }
            }
            Self::Regex(r) => write!(f, "Regex \"{}\"", r.as_str()),
            Self::Numerical(n) => write!(f, "{n}"),
            Self::Multiple(m) => {
                write!(f, "one of [{}]", m.iter().map(Value::to_string).join(", "))
            }
            Self::MultiplePartial(m) => {
                write!(f, "includes one of [{}]", m.join(", "))
            }
            Self::None => f.write_str("none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("t=pyro" => Ok(("", RawCardFilter(Field::Type, Operator::Equal, Value::String("pyro".into())))))]
    #[test_case("t:PYro" => Ok(("", RawCardFilter(Field::Type, Operator::Equal, Value::String("pyro".into())))); "input is lowercased")]
    #[test_case("t==warrior" => Ok(("", RawCardFilter(Field::Type, Operator::Equal, Value::String("warrior".into())))))]
    #[test_case("atk>=100" => Ok(("", RawCardFilter(Field::Atk, Operator::GreaterEqual, Value::Numerical(100)))))]
    #[test_case("Necrovalley" => Ok(("", RawCardFilter(Field::Name, Operator::Equal, Value::String("necrovalley".into())))))]
    #[test_case("l=10" => Ok(("", RawCardFilter(Field::Level, Operator::Equal, Value::Numerical(10)))))]
    #[test_case("Ib" => Ok(("", RawCardFilter(Field::Name, Operator::Equal, Value::String("ib".to_owned())))))]
    #[test_case("p<150" => Ok(("", RawCardFilter(Field::Price, Operator::Less, Value::Numerical(150)))))]
    fn successful_parsing_test(input: &str) -> IResult<&str, RawCardFilter> {
        parse_raw_filter(input)
    }

    #[test]
    fn sequential_parsing_test() {
        let (rest, filter) = parse_raw_filter("atk>=100 l:4").unwrap();
        assert_eq!(filter, RawCardFilter(Field::Atk, Operator::GreaterEqual, Value::Numerical(100)));
        assert_eq!(parse_raw_filter(rest), Ok(("", RawCardFilter(Field::Level, Operator::Equal, Value::Numerical(4)))));

        assert_eq!(
            parse_raw_filters("atk>=100 l=4"),
            Ok((
                "",
                vec![
                    RawCardFilter(Field::Atk, Operator::GreaterEqual, Value::Numerical(100)),
                    RawCardFilter(Field::Level, Operator::Equal, Value::Numerical(4))
                ]
            ))
        );

        assert_eq!(
            parse_raw_filters(r#"t:counter t:trap o:"negate the summon""#),
            Ok((
                "",
                vec![
                    RawCardFilter(Field::Type, Operator::Equal, Value::String("counter".into())),
                    RawCardFilter(Field::Type, Operator::Equal, Value::String("trap".into())),
                    RawCardFilter(Field::Text, Operator::Equal, Value::String("negate the summon".into())),
                ]
            ))
        );
    }

    #[test]
    fn parse_multiple_values() {
        let input = "level=4|5|6";
        let expected_output = vec![RawCardFilter(
            Field::Level,
            Operator::Equal,
            Value::Multiple(vec![Value::Numerical(4), Value::Numerical(5), Value::Numerical(6)]),
        )];
        assert_eq!(parse_raw_filters(input), Ok(("", expected_output)));
    }

    #[test]
    fn quoted_value_test() {
        let (rest, filter) = parse_raw_filter(r#"o:"destroy that target""#).unwrap();
        assert_eq!(rest, "");
        assert_eq!(filter, RawCardFilter(Field::Text, Operator::Equal, Value::String("destroy that target".into())));
    }

    #[test]
    fn regex_should_have_precedence_over_split() {
        let RawCardFilter(field, op, value) = parse_raw_filters("o:/(if|when) this card is Synchro Summoned:/").unwrap().1[0].clone();
        assert_eq!(field, Field::Text);
        assert_eq!(op, Operator::Equal);
        match value {
            Value::Regex(r) => assert_eq!(r.as_str(), "(if|when) this card is synchro summoned:"),
            _ => panic!("Should have been a regex"),
        }
        let RawCardFilter(field, op, value) = parse_raw_filters("name:/(XYZ|pendulum|synchro|fusion) dragon/").unwrap().1[0].clone();
        assert_eq!(field, Field::Name);
        assert_eq!(op, Operator::Equal);
        match value {
            Value::Regex(r) => assert_eq!(r.as_str(), "(xyz|pendulum|synchro|fusion) dragon"),
            _ => panic!("Should have been a regex"),
        }
    }
}
