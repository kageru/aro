use serde::Deserialize;
use std::{
    fmt::{self, Display, Write},
    sync::LazyLock,
};
use time::Date;

use crate::{IMG_HOST, SETS_BY_NAME};

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct CardInfo {
    pub data: Vec<Card>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct Card {
    pub id:            usize,
    pub typeline:      Option<Vec<String>>,
    #[serde(rename = "humanReadableCardType")]
    pub type_fallback: String, // For Spell/Trap cards
    pub name:          String,
    #[serde(rename = "desc")]
    pub text:          String,
    // Will be -1 for ?
    pub atk:           Option<i32>,
    pub def:           Option<i32>,
    pub attribute:     Option<String>,
    // also includes rank
    pub level:         Option<i32>,
    #[serde(rename = "linkval")]
    pub link_rating:   Option<i32>,
    #[serde(rename = "linkmarkers")]
    pub link_arrows:   Option<Vec<String>>,
    #[serde(default)]
    pub card_sets:     Vec<CardSet>,
    pub banlist_info:  Option<BanlistInfo>,
    #[serde(default)]
    pub card_prices:   Vec<CardPrice>,
    pub misc_info:     Vec<MiscInfo>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
pub struct BanlistInfo {
    #[serde(default)]
    pub ban_tcg: BanlistStatus,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct MiscInfo {
    pub beta_name:      Option<String>,
    pub treated_as:     Option<String>,
    pub tcg_date:       Option<Date>,
    pub genesys_points: i32,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
pub enum BanlistStatus {
    Forbidden = 0,
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

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct CardPrice {
    pub cardmarket_price: String,
    pub tcgplayer_price:  String,
}

impl Card {
    pub fn extended_info(&self) -> Result<String, fmt::Error> {
        let mut s = String::with_capacity(1000);
        // the ygorg search breaks for I:P and similar criminals.
        let url_name = self.name.replace(':', " ");
        write!(
            s,
            "<p><a href=\"https://db.ygorganization.com/search#card:{url_name}\">Rulings</a> – <a href=\"https://yugipedia.com/wiki/{:08}\">Yugipedia</a></p>",
            &self.id
        )?;
        s.push_str("<h3>Printings:</h3>");
        for printing in &self.card_sets {
            write!(s, "{}: {} ({})", printing.set_name, printing.set_code, printing.set_rarity)?;
            if let Some(date) = SETS_BY_NAME.get(&printing.set_name.to_lowercase()).and_then(|s| s.tcg_date) {
                write!(s, " - {date}")?;
            }
            s.push_str("<br/>");
        }
        if let Some(CardPrice { cardmarket_price, tcgplayer_price }) = self.card_prices.first() {
            s.push_str("<h3>Prices:</h3>");
            write!(
                s,
                "Cardmarket: <a href=\"https://www.cardmarket.com/en/YuGiOh/Products/Search?searchString={url_name}\">{cardmarket_price}&ThinSpace;€</a><br/>"
            )?;
            write!(
                s,
                "TCGplayer: <a href=\"https://www.tcgplayer.com/search/yugioh/product?productLineName=yugioh&q={url_name}\">$&ThinSpace;{tcgplayer_price}</a><br/>"
            )?;
        }
        Ok(s)
    }

    pub fn short_info(&self) -> Result<String, fmt::Error> {
        let mut s = String::new();
        s.push_str(&self.name);
        s.push('\n');
        self.basic_info(&mut s, "\n")?;
        Ok(s)
    }

    fn basic_info<W: Write>(&self, f: &mut W, newline: &str) -> fmt::Result {
        if let Some(lr) = self.link_rating {
            write!(f, "Link {lr} ")?;
        } else if let Some(level) = self.level {
            if let Some(t) = &self.typeline
                && t.contains(&String::from("XYZ"))
            {
                write!(f, "Rank {level} ")?;
            } else {
                write!(f, "Level {level} ")?;
            }
        }
        if let Some(attr) = &self.attribute {
            write!(f, "{attr}/")?;
        }
        match &self.typeline {
            Some(t) => write!(f, "{}", t.join(" ")),
            None => write!(f, "{}", self.type_fallback),
        }?;
        if self.type_fallback.contains(&String::from("Monster")) {
            f.write_str(newline)?;
            match (self.atk, self.def) {
                (Some(atk), Some(def)) => write!(f, "{} ATK / {} DEF", stat_display(atk), stat_display(def))?,
                (Some(atk), None) if self.link_rating.is_some() => write!(f, "{} ATK", stat_display(atk))?,
                _ => (),
            }
        }
        Ok(())
    }
}

fn stat_display(n: i32) -> String {
    match n {
        -1 => String::from("?"),
        _ => n.to_string(),
    }
}

static FORBIDDEN_ICON: LazyLock<String> =
    LazyLock::new(|| format!(r#"<img class="banlist-icon" src="{}/static/forbidden.svg"/>"#, IMG_HOST.as_str()));
static LIMITED_ICON: LazyLock<String> =
    LazyLock::new(|| format!(r#"<img class="banlist-icon" src="{}/static/limited.svg"/>"#, IMG_HOST.as_str()));
static SEMI_LIMITED_ICON: LazyLock<String> =
    LazyLock::new(|| format!(r#"<img class="banlist-icon" src="{}/static/semi-limited.svg"/>"#, IMG_HOST.as_str()));

impl Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"<h2><span class="cardname" title="{}">{}</span> {} {}</h2><em>"#,
            match &self.misc_info[0].beta_name {
                Some(name) => format!("Previously “{name}”"),
                None => String::new(),
            },
            &self.name,
            match self.banlist_info.map(|bi| bi.ban_tcg) {
                Some(BanlistStatus::Forbidden) => &FORBIDDEN_ICON,
                Some(BanlistStatus::Limited) => &LIMITED_ICON,
                Some(BanlistStatus::SemiLimited) => &SEMI_LIMITED_ICON,
                _ => "",
            },
            match self.misc_info[0].genesys_points {
                0 => String::new(),
                p => format!(r#"<span class="genesys">{}</span>"#, p),
            },
        )?;
        self.basic_info(f, "<br/>")?;
        write!(f, "</em><hr/><p>{}</p>", &self.text)?;
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
      "humanReadableCardType": "Normal Spell",
      "desc": "Discard up to 3 Monster Cards from your hand to the Graveyard.",
      "card_sets": [
        {
          "set_name": "Dark Beginning 1",
          "set_code": "DB1-EN167",
          "set_rarity": "Common",
          "set_rarity_code": "(C)",
          "set_price": "0"
        },
        {
          "set_name": "Metal Raiders",
          "set_code": "MRD-059",
          "set_rarity": "Common",
          "set_rarity_code": "(C)",
          "set_price": "1.55"
        }
      ],
      "card_prices": [
        {
          "cardmarket_price": "0.06",
          "tcgplayer_price": "0.10"
        }
      ],
      "misc_info": [
        {
          "tcg_date": "2002-06-26",
          "has_effect": 1,
          "genesys_points": 0
        }
      ]
    }
    "#;

    pub const RAW_MONSTER: &str = r#"
    {
      "id": 2326738,
      "name": "Des Lacooda",
      "typeline": [
        "Zombie",
        "Effect"
      ],
      "humanReadableCardType": "Effect Monster",
      "desc": "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card.",
      "atk": 500,
      "def": 600,
      "level": 3,
      "attribute": "EARTH",
      "card_sets": [
        {
          "set_name": "Astral Pack Three",
          "set_code": "AP03-EN018",
          "set_rarity": "Common",
          "set_rarity_code": "(C)",
          "set_price": "0"
        },
        {
          "set_name": "Gold Series",
          "set_code": "GLD1-EN010",
          "set_rarity": "Common",
          "set_rarity_code": "(C)",
          "set_price": "0"
        }

      ],
      "card_prices": [
        {
          "cardmarket_price": "0.22",
          "tcgplayer_price": "0.14"
        }
      ],
      "misc_info": [
        {
          "tcg_date": "2003-07-18",
          "has_effect": 1,
          "genesys_points": 0
        }
      ]
    }
    "#;

    pub const RAW_LINK_MONSTER: &str = r#"
    {
      "id": 49202162,
      "name": "Black Luster Soldier - Soldier of Chaos",
      "typeline": [
        "Warrior",
        "Link",
        "Effect"
      ],
      "humanReadableCardType": "Link Effect Monster",
      "desc": "3 monsters with different names\r\nIf this card was Link Summoned using a Level 7 or higher monster(s) as material, your opponent cannot target it with card effects, also it cannot be destroyed by your opponent's card effects. When this card destroys an opponent's monster by battle: You can activate 1 of these effects;\r\n● This card gains 1500 ATK.\r\n● This card can make a second attack during the Battle Phase of your next turn.\r\n● Banish 1 card on the field.",
      "atk": 3000,
      "def": null,
      "level": null,
      "attribute": "EARTH",
      "archetype": "Black Luster Soldier",
      "linkval": 3,
      "linkmarkers": [
        "Top",
        "Bottom-Left",
        "Bottom-Right"
      ],
      "card_sets": [
        {
          "set_name": "OTS Tournament Pack 17",
          "set_code": "OP17-EN003",
          "set_rarity": "Ultimate Rare",
          "set_rarity_code": "(UtR)",
          "set_price": "0"
        }
      ],
      "card_prices": [
        {
          "cardmarket_price": "0.55",
          "tcgplayer_price": "2.60"
        }
      ],
      "misc_info": [
        {
          "beta_name": "Black Luster Soldier, the Chaos Warrior",
          "tcg_date": "2019-07-11",
          "has_effect": 1,
          "genesys_points": 0
        }
      ]
    }
    "#;

    #[test]
    fn test_spell() {
        let coffin: Card = serde_json::from_str(RAW_SPELL).unwrap();
        assert_eq!(
            coffin,
            Card {
                id: 41142615,
                type_fallback: "Normal Spell".to_owned(),
                name: "The Cheerful Coffin".to_owned(),
                text: "Discard up to 3 Monster Cards from your hand to the Graveyard.".to_owned(),
                card_sets: vec![
                    CardSet {
                        set_name:   "Dark Beginning 1".to_owned(),
                        set_code:   "DB1-EN167".to_owned(),
                        set_rarity: "Common".to_owned(),
                    },
                    CardSet { set_name: "Metal Raiders".to_owned(), set_code: "MRD-059".to_owned(), set_rarity: "Common".to_owned() }
                ],
                card_prices: vec![CardPrice { tcgplayer_price: "0.10".to_owned(), cardmarket_price: "0.06".to_owned() }],
                misc_info: vec![MiscInfo { beta_name: None, treated_as: None, tcg_date: Some(Date::from_calendar_date(2002, time::Month::June, 26).unwrap()), genesys_points: 0 }],
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
                typeline: Some(vec!["Zombie".to_owned(), "Effect".to_owned()]),
                name: "Des Lacooda".to_owned(),
                type_fallback: "Effect Monster".to_owned(),
                text:
                    "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card."
                        .to_owned(),
                atk: Some(500),
                def: Some(600),
                level: Some(3),
                attribute: Some("EARTH".to_owned()),
                card_sets: vec![
                    CardSet {
                        set_name:   "Astral Pack Three".to_owned(),
                        set_code:   "AP03-EN018".to_owned(),
                        set_rarity: "Common".to_owned(),
                    },
                    CardSet { set_name: "Gold Series".to_owned(), set_code: "GLD1-EN010".to_owned(), set_rarity: "Common".to_owned() }
                ],
                card_prices: vec![CardPrice { tcgplayer_price: "0.14".to_owned(), cardmarket_price: "0.22".to_owned() }],
                misc_info: vec![MiscInfo {
                    beta_name:      None,
                    treated_as:     None,
                    tcg_date:       Some(Date::from_calendar_date(2003, time::Month::July, 18).unwrap()),
                    genesys_points: 0,
                }],
                ..Default::default()
            },
        )
    }
}
