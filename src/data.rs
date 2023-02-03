use serde::Deserialize;
use std::fmt::{self, Display, Write};
use time::Date;

use crate::SETS_BY_NAME;

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct CardInfo {
    pub data: Vec<Card>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct Card {
    pub id:           usize,
    #[serde(rename = "type")]
    pub card_type:    String,
    pub name:         String,
    #[serde(rename = "desc")]
    pub text:         String,
    // Will also be None for ?
    pub atk:          Option<i32>,
    pub def:          Option<i32>,
    pub attribute:    Option<String>,
    #[serde(rename = "race")]
    pub r#type:       String,
    // also includes rank
    pub level:        Option<i32>,
    #[serde(rename = "linkval")]
    pub link_rating:  Option<i32>,
    #[serde(rename = "linkmarkers")]
    pub link_arrows:  Option<Vec<String>>,
    #[serde(default)]
    pub card_sets:    Vec<CardSet>,
    pub banlist_info: Option<BanlistInfo>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
pub struct BanlistInfo {
    #[serde(default)]
    pub ban_tcg: BanlistStatus,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
pub enum BanlistStatus {
    Banned = 0,
    Limited = 1,
    #[serde(rename = "Semi-Limited")]
    SemiLimited = 2,
    #[default]
    Unlimited = 3,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct CardSet {
    pub set_name:   String,
    pub set_code:   String,
    pub set_rarity: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct Set {
    pub set_name: String,
    pub tcg_date: Option<Date>,
}

impl Card {
    pub fn extended_info(&self) -> Result<String, fmt::Error> {
        let mut s = String::with_capacity(1000);
        s.push_str("<h3>Printings:</h3>");
        for printing in &self.card_sets {
            write!(s, "{}: {} ({})", printing.set_name, printing.set_code, printing.set_rarity)?;
            if let Some(date) = SETS_BY_NAME.get(&printing.set_name.to_lowercase()).and_then(|s| s.tcg_date) {
                write!(s, " - {}", date)?;
            }
            s.push_str("<br/>");
        }
        Ok(s)
    }
}

impl Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, r#"<h2 class="cardname">{}</h2><br/><em>"#, &self.name)?;
        if let Some(level) = self.level {
            if self.card_type.contains("XYZ") {
                f.write_str("Rank ")?;
            } else {
                f.write_str("Level ")?;
            }
            write!(f, "{level} ")?;
        } else if let Some(lr) = self.link_rating {
            write!(f, "Link {lr} ")?;
        }
        if let Some(attr) = &self.attribute {
            write!(f, "{attr}/")?;
        }
        write!(f, "{} {}", self.r#type, self.card_type)?;
        if self.card_type.contains(&String::from("Monster")) {
            f.write_str("<br/>")?;
            match (self.atk, self.def) {
                (Some(atk), Some(def)) => write!(f, "{atk} ATK / {def} DEF")?,
                (Some(atk), None) if self.link_rating.is_some() => write!(f, "{atk} ATK")?,
                (None, Some(def)) => write!(f, "? ATK / {def} DEF")?,
                (Some(atk), None) => write!(f, "{atk} ATK / ? DEF")?,
                (None, None) => write!(f, "? ATK / ? DEF")?,
            }
        }
        write!(f, "</em><p>{}</p>", &self.text)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const RAW_SPELL: &str = r#"
    {
      "id": 41142615,
      "name": "The Cheerful Coffin",
      "type": "Spell Card",
      "desc": "Discard up to 3 Monster Cards from your hand to the Graveyard.",
      "race": "Normal",
      "card_sets": [
        {
          "set_name": "Dark Beginning 1",
          "set_code": "DB1-EN167",
          "set_rarity": "Common",
          "set_rarity_code": "(C)",
          "set_price": "1.41"
        },
        {
          "set_name": "Metal Raiders",
          "set_code": "MRD-059",
          "set_rarity": "Common",
          "set_rarity_code": "(C)",
          "set_price": "1.55"
        }
      ]
    }"#;

    pub const RAW_MONSTER: &str = r#"
    {
       "id": 2326738,
       "name": "Des Lacooda",
       "type": "Effect Monster",
       "desc": "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card.",
       "atk": 500,
       "def": 600,
       "level": 3,
       "race": "Zombie",
       "attribute": "EARTH",
       "card_sets": [
         {
           "set_name": "Astral Pack Three",
           "set_code": "AP03-EN018",
           "set_rarity": "Common",
           "set_rarity_code": "(C)",
           "set_price": "1.24"
         },
         {
           "set_name": "Gold Series",
           "set_code": "GLD1-EN010",
           "set_rarity": "Common",
           "set_rarity_code": "(C)",
           "set_price": "2.07"
         }
       ]
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
                card_sets: vec![
                    CardSet {
                        set_name:   "Dark Beginning 1".to_owned(),
                        set_code:   "DB1-EN167".to_owned(),
                        set_rarity: "Common".to_owned(),
                    },
                    CardSet { set_name: "Metal Raiders".to_owned(), set_code: "MRD-059".to_owned(), set_rarity: "Common".to_owned() }
                ],
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
                card_sets: vec![
                    CardSet {
                        set_name:   "Astral Pack Three".to_owned(),
                        set_code:   "AP03-EN018".to_owned(),
                        set_rarity: "Common".to_owned(),
                    },
                    CardSet { set_name: "Gold Series".to_owned(), set_code: "GLD1-EN010".to_owned(), set_rarity: "Common".to_owned() }
                ],
                ..Default::default()
            },
        )
    }
}
