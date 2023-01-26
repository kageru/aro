#![feature(option_result_contains)]
use std::{collections::HashMap, time::Instant};

use data::CardInfo;
use filter::SearchCard;

mod data;
mod filter;
mod parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cards = serde_json::from_reader::<_, CardInfo>(std::io::BufReader::new(std::fs::File::open("cards.json")?))?.data;
    let search_cards: Vec<_> = cards.iter().map(SearchCard::from).collect();
    let cards_by_id: HashMap<_, _> = cards.into_iter().map(|c| (c.id, c)).collect();
    let now = Instant::now();
    let raw_query = std::env::args().nth(1).unwrap();
    let query = parser::parse_filters(&raw_query)?;
    let query_time = now.elapsed();
    let now = Instant::now();
    let matches: Vec<_> = search_cards.iter().filter(|card| query.iter().all(|q| q(card))).collect();
    let filter_time = now.elapsed();
    for c in &matches {
        println!("{}\n", cards_by_id.get(&c.id).unwrap());
    }
    println!("Parsed query in {:?}", query_time);
    println!("Searched {} cards in {:?} ({} hits)", search_cards.len(), filter_time, matches.len());
    Ok(())
}
