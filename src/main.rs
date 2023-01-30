#![feature(option_result_contains, once_cell)]
use actix_web::{get, http::header, web, App, Either, HttpResponse, HttpServer};
use data::{Card, CardInfo};
use filter::SearchCard;
use itertools::Itertools;
use serde::Deserialize;
use std::{collections::HashMap, fmt::Write, fs::File, io::BufReader, net::Ipv4Addr, sync::LazyLock, time::Instant};

mod data;
mod filter;
mod parser;

const RESULT_LIMIT: usize = 100;

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
    HttpServer::new(|| App::new().service(search).service(card_info).service(help))
        .bind((Ipv4Addr::from([127, 0, 0, 1]), 1961))?
        .run()
        .await
}

#[derive(Debug, Deserialize)]
struct Query {
    q: String,
}

const HEADER: &str = include_str!("../static/header.html");
const HELP_CONTENT: &str = include_str!("../static/help.html");
const FOOTER: &str = r#"<div id="bottom">
<a href="/">Home</a>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="/help">Query Syntax</a>
</div>
</body></html>"#;

#[get("/")]
async fn search(q: Option<Either<web::Query<Query>, web::Form<Query>>>) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let q = match q {
        Some(Either::Left(web::Query(Query { q }))) => Some(q),
        Some(Either::Right(web::Form(Query { q }))) => Some(q),
        None => None,
    }
    .filter(|s| !s.is_empty());
    let mut res = String::with_capacity(10_000);
    res.push_str(HEADER);
    render_searchbox(&mut res, &q)?;
    match q {
        Some(q) => render_results(&mut res, &q)?,
        None => res.push_str("Enter a query above to search"),
    }
    finish_document(&mut res);
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}

#[get("/card/{id}")]
async fn card_info(card_id: web::Path<usize>) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let mut res = String::with_capacity(2_000);
    res.push_str(HEADER);
    render_searchbox(&mut res, &None)?;
    match CARDS_BY_ID.get(&card_id) {
        Some(card) => {
            res.push_str(r#""#);
            write!(
                res,
                r#"
<div class="row">
    <div class="column left">{card}</div>
    <div class="column right"><img style="width: 100%;" src="/static/full/{}.jpg"/></div>
</div>"#,
                card.id,
            )?;
        }
        None => res.push_str("Card not found"),
    }
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}

#[get("/help")]
async fn help() -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let mut res = String::with_capacity(HEADER.len() + HELP_CONTENT.len() + FOOTER.len() + 250);
    res.push_str(HEADER);
    render_searchbox(&mut res, &None)?;
    res.push_str(HELP_CONTENT);
    res.push_str(FOOTER);
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}

fn render_searchbox(res: &mut String, query: &Option<String>) -> std::fmt::Result {
    write!(
        res,
        r#"
<form action="/">
  <input type="text" name="q" id="searchbox" placeholder="Enter query (e.g. l:5 c:synchro atk>2000)" value="{}"><input type="submit" id="submit" value="🔍">
</form>"#,
        match &query {
            Some(q) => q,
            None => "",
        }
    )
}

fn render_results(res: &mut String, query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let query = match parser::parse_filters(query) {
        Ok(q) => q,
        Err(e) => {
            write!(res, "Could not parse query: {e:?}")?;
            return Ok(());
        }
    };
    let now = Instant::now();
    let matches: Vec<&Card> = SEARCH_CARDS
        .iter()
        .filter(|card| query.iter().all(|(_, q)| q(card)))
        .map(|c| CARDS_BY_ID.get(&c.id).unwrap())
        .take(RESULT_LIMIT)
        .collect();
    write!(
        res,
        "<span class=\"meta\">Showing {} results where {} (took {:?})</span>",
        matches.len(),
        query.iter().map(|(f, _)| f.to_string()).join(" and "),
        now.elapsed()
    )?;
    if matches.is_empty() {
        return Ok(());
    }
    res.push_str("<table>");
    for card in matches {
        write!(
            res,
            r#"<tr><td>{card}</td><td><a href="/card/{}"><img src="/static/thumb/{}.jpg" class="thumb"/></a></td></tr>"#,
            card.id, card.id
        )?;
    }
    Ok(())
}

fn finish_document(res: &mut String) {
    res.push_str(FOOTER)
}
