use serde::{de::Visitor, Deserialize, Deserializer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cards: CardInfo =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open("cards.json")?))?;
    println!("{} cards read", cards.data.len());
    Ok(())
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
struct CardInfo {
    data: Vec<Card>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
struct CardBase {
    //#[serde(rename = "type", deserialize_with = "split_types")]
    //card_type: Vec<String>,
    name: String,
    #[serde(rename = "desc")]
    text: String,
}

fn split_types<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    struct SplittingVisitor;

    impl<'de> Visitor<'de> for SplittingVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.split_whitespace()
                .filter(|t| t != &"Card")
                .map(str::to_owned)
                .collect())
        }
    }
    deserializer.deserialize_any(SplittingVisitor)
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
struct Monster {
    // None for ?
    atk: Option<i32>,
    attribute: String,
    #[serde(rename = "race")]
    r#type: String,
    // None for ? or link monsters
    def: Option<i32>,
    // also includes rank
    level: Option<u8>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "type")]
enum Card {
    #[serde(alias = "Spell Card", alias = "Trap Card")]
    Backrow {
        #[serde(flatten)]
        base: CardBase,
    },
    #[serde(rename = "Skill Card")]
    Skill {
        #[serde(flatten)]
        base: CardBase,
    },
    #[serde(
        alias = "Effect Monster",
        alias = "Flip Effect Monster",
        alias = "Fusion Monster",
        alias = "Gemini Monster",
        alias = "Link Monster",
        alias = "Normal Monster",
        alias = "Normal Tuner Monster",
        alias = "Pendulum Effect Fusion Monster",
        alias = "Pendulum Effect Monster",
        alias = "Pendulum Effect Ritual Monster",
        alias = "Pendulum Flip Effect Monster",
        alias = "Pendulum Normal Monster",
        alias = "Pendulum Tuner Effect Monster",
        alias = "Ritual Effect Monster",
        alias = "Ritual Monster",
        alias = "Spirit Monster",
        alias = "Synchro Monster",
        alias = "Synchro Pendulum Effect Monster",
        alias = "Synchro Tuner Monster",
        alias = "Token",
        alias = "Toon Monster",
        alias = "Tuner Monster",
        alias = "Union Effect Monster",
        alias = "XYZ Monster",
        alias = "XYZ Pendulum Effect Monster"
    )]
    Monster {
        #[serde(flatten)]
        base: CardBase,
        #[serde(flatten)]
        monster: Monster,
        #[serde(default, rename = "linkval")]
        link_rating: u8,
        #[serde(default)]
        linkmarkers: Vec<String>,
    },
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
            Card::Backrow {
                base: CardBase {
                    card_type: vec!["Spell".to_owned()],
                    name: "The Cheerful Coffin".to_owned(),
                    text: "Discard up to 3 Monster Cards from your hand to the Graveyard."
                        .to_owned()
                }
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
            Card::Monster {
                base: CardBase {
                    card_type: vec!["Effect".to_owned(), "Monster".to_owned()],
                    name: "Des Lacooda".to_owned(),
                    text: "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card.".to_owned(),
                },
                monster: Monster {
                    atk: Some(500),
                    def: Some(600),
                    level: Some(3),
                    r#type: "Zombie".to_owned(),
                    attribute: "EARTH".to_owned(),
                },
                link_rating: 0,
                linkmarkers: vec![]
            },
        )
    }
}
