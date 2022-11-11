use crossterm::terminal::enable_raw_mode;
use serde::Deserialize;
use std::{io, iter, time::Duration};
use tui::{
    backend::CrosstermBackend,
    layout,
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

type Terminal = tui::Terminal<CrosstermBackend<io::Stdout>>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    enable_raw_mode()?;
    let mut backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    let all_cards= vec![
        Card::SpellCard {
            name: "The Cheerful Coffin".to_owned(),
            text: "Discard up to 3 Monster Cards from your hand to the Graveyard.".to_owned()
        },
        Card::EffectMonster {
            name: "Des Lacooda".to_owned(),
            effect: "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card.".to_owned(),
            atk: 500,
            def: 600,
            level: 3,
            r#type: "Zombie".to_owned(),
            attribute: "EARTH".to_owned(),
        }
        ];
    let mut cards = all_cards.clone();
    let mut search_text = String::new();

    loop {
        refresh(&mut terminal, &cards, &search_text);
    }
    Ok(())
}

fn refresh(term: &mut Terminal, cards: &Vec<Card>, search_text: &str) -> Result<(), io::Error> {
    let mut list = selectable_list(
        search_text,
        cards
            .iter()
            .map(|c| ListItem::new(format!("{c:?}")))
            .collect::<Vec<_>>(),
        None,
    );
    term.draw(|mut frame| {
        frame.render_widget(list, frame.size());
    })?;
    Ok(())
}

pub fn selectable_list<'a>(
    title: &'a str,
    content: Vec<ListItem<'a>>,
    selected: Option<usize>,
) -> List<'a> {
    List::new(content)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT),
        )
        .highlight_style(Style::default().fg(Color::LightGreen))
        .highlight_symbol(">")
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "type")]
enum Card {
    #[serde(rename = "Spell Card")]
    SpellCard {
        name: String,
        #[serde(rename = "desc")]
        text: String,
    },
    #[serde(
        rename = "Effect Monster",
        alias = "Flip Effect Monster",
        alias = "Union Effect Monster"
    )]
    EffectMonster {
        name: String,
        #[serde(rename = "desc")]
        effect: String,
        atk: i32,
        def: i32,
        level: i32,
        attribute: String,
        #[serde(rename = "race")]
        r#type: String,
    },
}

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
        Card::SpellCard {
            name: "The Cheerful Coffin".to_owned(),
            text: "Discard up to 3 Monster Cards from your hand to the Graveyard.".to_owned()
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
        Card::EffectMonster {
            name: "Des Lacooda".to_owned(),
            effect: "Once per turn: You can change this card to face-down Defense Position. When this card is Flip Summoned: Draw 1 card.".to_owned(),
            atk: 500,
            def: 600,
            level: 3,
            r#type: "Zombie".to_owned(),
            attribute: "EARTH".to_owned(),
        }
    )
}
