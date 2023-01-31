use serde::Deserialize;
use std::fmt::{self, Display};

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct CardInfo {
    pub data: Vec<Card>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct Card {
    pub id:          usize,
    #[serde(rename = "type")]
    pub card_type:   String,
    pub name:        String,
    #[serde(rename = "desc")]
    pub text:        String,
    // Will also be None for ?
    pub atk:         Option<i32>,
    pub def:         Option<i32>,
    pub attribute:   Option<String>,
    #[serde(rename = "race")]
    pub r#type:      String,
    // also includes rank
    pub level:       Option<i32>,
    #[serde(rename = "linkval")]
    pub link_rating: Option<i32>,
    #[serde(rename = "linkmarkers")]
    pub link_arrows: Option<Vec<String>>,
}

impl Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, r#"<h2><a href="/card/{}">{}</a></h2><br/><em>"#, &self.id, &self.name)?;
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
      "race": "Normal"
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
}
