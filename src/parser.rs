use std::{
    fmt::{self, Display},
    str::FromStr,
};

use crate::filter::{build_filter, CardFilter};
use itertools::Itertools;
use nom::{
    branch::alt,
    bytes::complete::{take_until1, take_while, take_while_m_n},
    character::complete::{char, multispace0},
    combinator::{complete, map, map_res, recognize, rest, verify},
    multi::{many_m_n, separated_list1},
    sequence::{delimited, preceded, tuple},
    IResult,
};

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
    verify(alt((take_until1(" "), rest)), |s: &str| s.len() >= 2)(input)
}

fn sanitize(query: &str) -> Result<String, String> {
    if query.contains(OPERATOR_CHARS) {
        Err(format!("Invalid query: {query}"))
    } else {
        Ok(query.to_lowercase())
    }
}

fn fallback_filter(query: &str) -> Result<RawCardFilter, String> {
    let q = sanitize(query)?;
    Ok(RawCardFilter(Field::Name, Operator::Equal, Value::String(q)))
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
            recognize(separated_list1(char('|'), take_until1(" |"))),
            take_until1(" "),
            rest,
        )),
        |i: &str| {
            if i.contains('|') {
                let items: Vec<_> = i.split('|').collect();
                let mut values = Vec::new();

                for item in items {
                    match item.parse::<i32>() {
                        Ok(n) => values.push(Value::Numerical(n)),
                        Err(_) => values.push(Value::String(sanitize(item)?)),
                    }
                }

                Ok(Value::Multiple(values))
            } else {
                match i.parse() {
                    Ok(n) => Ok(Value::Numerical(n)),
                    Err(_) if i.is_empty() => Err("empty filter argument".to_string()),
                    Err(_) => Ok(Value::String(sanitize(i)?)),
                }
            }
        },
    )(input)
}

/// Ordinals are given highest = fastest to filter.
/// This is used to sort filters before applying them.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Field {
    Atk = 1,
    Def = 2,
    Legal = 3,
    Level = 4,
    LinkRating = 6,
    Year = 8,
    Set = 10,
    Type = 12,
    Attribute = 14,
    Class = 16,
    Name = 18,
    Text = 20,
}

impl Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "text",
            Self::Name => "name",
            Self::Class => "card type",
            Self::Attribute => "attribute",
            Self::Type => "type",
            Self::Level => "level/rank",
            Self::Atk => "ATK",
            Self::Def => "DEF",
            Self::LinkRating => "link rating",
            Self::Set => "set",
            Self::Year => "year",
            Self::Legal => "allowed copies",
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
            "c" | "class" => Self::Class,
            "o" | "eff" | "text" | "effect" | "e" => Self::Text,
            "lr" | "linkrating" => Self::LinkRating,
            "name" => Self::Name,
            "set" | "s" => Self::Set,
            "year" | "y" => Self::Year,
            "legal" | "copies" => Self::Legal,
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Value {
    String(String),
    Numerical(i32),
    Multiple(Vec<Value>),
}

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
            Self::Numerical(n) => write!(f, "{n}"),
            Self::Multiple(m) => {
                for v in m {
                    write!(f, "{v} or ")?
                }
                Ok(())
            }
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
    #[test_case("c!=synchro" => Ok(("", RawCardFilter(Field::Class, Operator::NotEqual, Value::String("synchro".to_owned())))))]
    fn successful_parsing_test(input: &str) -> IResult<&str, RawCardFilter> {
        parse_raw_filter(input)
    }

    #[test_case("atk<=>1")]
    #[test_case("l===10")]
    #[test_case("t=")]
    #[test_case("=100")]
    #[test_case("a")]
    fn unsuccessful_parsing_test(input: &str) {
        if let Ok((filters, _)) = parse_filters(input) {
            assert!(false, "Should have failed, but parsed as {filters:?}");
        }
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
            parse_raw_filters(r#"t:counter c:trap o:"negate the summon""#),
            Ok((
                "",
                vec![
                    RawCardFilter(Field::Type, Operator::Equal, Value::String("counter".into())),
                    RawCardFilter(Field::Class, Operator::Equal, Value::String("trap".into())),
                    RawCardFilter(Field::Text, Operator::Equal, Value::String("negate the summon".into())),
                ]
            ))
        );
    }

    #[test]
    fn test_parse_raw_filters_with_multiple_values() {
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
}
