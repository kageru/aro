use std::time::Duration;

use serde::Deserialize;
use tui_realm_stdlib::List;
use tuirealm::{
    event::{Key, KeyEvent},
    props::TextSpan,
    terminal::TerminalBridge,
    tui::layout::{Constraint, Layout},
    Application, Component, Event, EventListenerCfg, MockComponent, NoUserEvent, PollStrategy,
    Update,
};

fn main() {
    let mut app: Application<Id, Msg, NoUserEvent> = Application::init(
        EventListenerCfg::default()
            .default_input_listener(Duration::from_millis(20))
            .poll_timeout(Duration::from_millis(10)),
    );
    app.mount(Id::MainList, Box::new(MyList {
        cards: vec![
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
        ],
        ..Default::default()
    }), vec![])
        .unwrap();
    app.active(&Id::MainList).unwrap();
    let model = Model {
        app,
        quit: false,
        redraw: true,
        terminal: TerminalBridge::new().unwrap(),
        state: State::default(),
    };
    model.terminal.enter_alternate_screen();
    model.terminal.enable_raw_mode();

    while !model.quit {
        match app.tick(PollStrategy::Once) {
            Ok(messages) if messages.len() > 0 => {
                model.redraw = true;
                for msg in messages.into_iter() {
                    model.update(Some(msg));
                }
            }
            _ => {}
        }
        if model.redraw {
            model.view(&mut app);
            model.redraw = false;
        }
    }

    model.terminal.leave_alternate_screen();
    model.terminal.disable_raw_mode();
    model.terminal.clear_screen();
}

#[derive(Debug, PartialEq)]
enum Msg {
    Quit,
    SearchInput(char),
    SearchClear,
    Up,
    Down,
    Increment,
    Decrement,
}

#[derive(Debug, PartialEq, Hash, Clone, Copy, Eq)]
enum Id {
    MainList,
    SearchField,
}

#[derive(MockComponent, Default)]
struct MyList {
    component: List,
    cards: Vec<Card>,
}

impl Component<Msg, NoUserEvent> for MyList {
    fn on(&mut self, ev: tuirealm::Event<NoUserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(KeyEvent {
                code: Key::Char(ch),
                ..
            }) => match ch {
                '+' => Some(Msg::Increment),
                '-' => Some(Msg::Decrement),
                ch => {
                    self.component = self.component
                        .rows(
                        self.cards
                            .iter()
                            .map(|c| {
                                vec![TextSpan::new(match c {
                                    Card::SpellCard { name, .. } => name,
                                    Card::EffectMonster { name, .. } => name,
                                })]
                            })
                            .collect(),
                    );
                    Some(Msg::SearchInput(ch))
                }
            },
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => Some(Msg::Quit),
            Event::Keyboard(KeyEvent {
                code: Key::Backspace,
                ..
            }) => Some(Msg::SearchClear),
            _ => None,
        }
    }
}

struct Model {
    state: State,
    app: Application<Id, Msg, NoUserEvent>,
    terminal: TerminalBridge,
    quit: bool,
    redraw: bool,
}

impl Model {
    pub fn view(&mut self, app: &mut Application<Id, Msg, NoUserEvent>) {
        self.terminal
            .raw_mut()
            .draw(|f| {
                let chunks = Layout::default()
                    .constraints([Constraint::Percentage(100)])
                    .split(f.size());
                app.view(&Id::MainList, f, chunks[0]);
            })
            .unwrap();
    }
}

impl Update<Msg> for Model {
    fn update(&mut self, msg: Option<Msg>) -> Option<Msg> {
        self.redraw = true;
        match msg {
            Some(Msg::SearchInput(c)) => self.state.filter.push(c),
            Some(Msg::SearchClear) => self.state.filter.clear(),
            Some(Msg::Quit) => self.quit = true,
            _ => (),
        };
        None
    }
}

#[derive(PartialEq, Eq, Default, Debug)]
struct State {
    filter: String,
}

struct ApiResponse {
    data: Vec<Card>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
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
