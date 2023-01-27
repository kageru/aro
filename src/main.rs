#![feature(option_result_contains, once_cell)]
use actix_web::{get, http::header, web, App, Either, HttpResponse, HttpServer};
use data::{Card, CardInfo};
use filter::SearchCard;
use serde::Deserialize;
use std::{collections::HashMap, fmt::Write, fs::File, io::BufReader, net::Ipv4Addr, sync::LazyLock, time::Instant};

mod data;
mod filter;
mod parser;

static CARDS: LazyLock<Vec<Card>> = LazyLock::new(|| {
    serde_json::from_reader::<_, CardInfo>(BufReader::new(File::open("cards.json").expect("cards.json not found")))
        .expect("Could not deserialize cards")
        .data
});
static CARDS_BY_ID: LazyLock<HashMap<usize, Card>> =
    LazyLock::new(|| CARDS.iter().map(|c| (c.id, Card { text: c.text.replace("\r", "").replace('\n', "<br/>"), ..c.clone() })).collect());
static SEARCH_CARDS: LazyLock<Vec<SearchCard>> = LazyLock::new(|| CARDS.iter().map(SearchCard::from).collect());

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let now = Instant::now();
    println!("Starting server");
    // tap these so they’re initialized
    let num_cards = (CARDS_BY_ID.len() + SEARCH_CARDS.len()) / 2;
    println!("Read {num_cards} cards in {:?}", now.elapsed());
    HttpServer::new(|| App::new().service(search)).bind((Ipv4Addr::from([127, 0, 0, 1]), 8080))?.run().await
}

#[derive(Debug, Deserialize)]
struct Query {
    q: String,
}

#[get("/")]
async fn search(q: Option<Either<web::Query<Query>, web::Form<Query>>>) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let q = match q {
        Some(Either::Left(web::Query(Query { q }))) => Some(q),
        Some(Either::Right(web::Form(Query { q }))) => Some(q),
        None => None,
    };
    let mut res = String::with_capacity(10_000);
    write!(
        res,
        r#"
<html><body>
<form action="/">
  <label for="fname">Search query:</label><br>
  <input type="text" name="q" value="{}"><br>
  <input type="submit" value="Submit">
</form>"#,
        match &q {
            Some(q) => q,
            None => "",
        }
    )?;
    if let Some(q) = q {
        let query = parser::parse_filters(&q)?;
        let now = Instant::now();
        let matches: Vec<&Card> =
            SEARCH_CARDS.iter().filter(|card| query.iter().all(|q| q(card))).map(|c| CARDS_BY_ID.get(&c.id).unwrap()).collect();
        write!(res, "Showing {} results (took {:?})<br/><br/>", matches.len(), now.elapsed())?;
        for card in matches {
            res.push_str(&card.to_string());
            res.push_str("<br/><br/>");
        }
        write!(res, "</body></html>")?;
    } else {
        res.write_str("Enter a query above to search")?;
    }
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}
