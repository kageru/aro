#![feature(lazy_cell, try_blocks)]
use actix_web::{http::header, route, web, App, Either, HttpResponse, HttpServer};
use data::{Card, CardInfo, Set};
use filter::SearchCard;
use itertools::Itertools;
use regex::{Captures, Regex};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::Write,
    fs::File,
    io::BufReader,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        LazyLock,
    },
    time::Instant,
};
use time::Date;

mod data;
mod filter;
mod parser;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

// The yearly tins have ~250 cards in them.
// I want to be higher than that so the page is usable as a set list.
const RESULT_LIMIT: usize = 300;

static CARDS: LazyLock<Vec<Card>> = LazyLock::new(|| {
    let mut cards = serde_json::from_reader::<_, CardInfo>(BufReader::new(File::open("cards.json").expect("cards.json not found")))
        .expect("Could not deserialize cards")
        .data;
    cards.iter_mut().for_each(|c| {
        c.card_sets.sort_unstable_by_key(|s| SETS_BY_NAME.get(&s.set_name.to_lowercase()).and_then(|s| s.tcg_date).unwrap_or(Date::MAX))
    });
    cards
});
static CARDS_BY_ID: LazyLock<HashMap<usize, Card>> = LazyLock::new(|| {
    CARDS
        .iter()
        .map(|c| {
            let text = PENDULUM_SEPARATOR
                .replacen(&c.text.replace('\r', ""), 1, |caps: &Captures| {
                    format!("</p><hr/>[ {} ]<p>", caps.iter().flatten().last().map_or_else(|| "Monster Effect", |g| g.as_str()))
                })
                .replace('\n', "<br/>");
            (c.id, Card { text, ..c.clone() })
        })
        .collect()
});
static SEARCH_CARDS: LazyLock<Vec<SearchCard>> = LazyLock::new(|| CARDS.iter().map(SearchCard::from).collect());
static SETS_BY_NAME: LazyLock<HashMap<String, Set>> = LazyLock::new(|| {
    serde_json::from_reader::<_, Vec<Set>>(BufReader::new(File::open("sets.json").expect("sets.json not found")))
        .expect("Could not deserialize sets")
        .into_iter()
        .map(|s| (s.set_name.to_lowercase(), s))
        .collect()
});
static PENDULUM_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(\\n-+)?\\n\\[\\s?(Monster Effect|Flavor Text)\\s?\\]\\n?").unwrap());

static IMG_HOST: LazyLock<String> = LazyLock::new(|| std::env::var("IMG_HOST").unwrap_or_else(|_| String::new()));

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

#[derive(Debug)]
enum TargetPage {
    Data(PageData),
    Redirect(String),
}

#[derive(Debug)]
struct PageData {
    description: String,
    title:       String,
    query:       Option<String>,
    body:        String,
}

const NAME: &str = "Unofficial YGO Card Search";
const HEADER: &str = include_str!("../static/header.html");
const HELP_CONTENT: &str = include_str!("../static/help.html");
static VIEW_COUNT: AtomicUsize = AtomicUsize::new(0);
fn footer() -> String {
    format!(
        r#"<div id="bottom">
<span style="color: #bbb">{}</span>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="/">Home</a>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="/help">Query Syntax</a>
</div>
</body></html>"#,
        VIEW_COUNT.fetch_add(1, Ordering::Relaxed)
    )
}

#[route("/", method = "GET", method = "HEAD")]
async fn search(q: Option<Either<web::Query<Query>, web::Form<Query>>>) -> AnyResult<HttpResponse> {
    let q = match q {
        Some(Either::Left(web::Query(Query { q }))) => Some(q),
        Some(Either::Right(web::Form(Query { q }))) => Some(q),
        None => None,
    }
    .filter(|s| !s.is_empty());
    let mut res = String::with_capacity(10_000);
    let data = match q {
        Some(q) => compute_results(q)?,
        None => TargetPage::Data(PageData {
            title:       "YGO card search".to_owned(),
            description: "Enter a query above to search".to_owned(),
            query:       None,
            body:        "<p>Welcome to my cheap Scryfall clone for Yugioh.</p>\
                          <p>Enter a query above to search or read the <a href=\"/help\">query syntax</a> for more information.</p>\
                          <p>The source code is available <a href=\"https://git.kageru.moe/kageru/aro\">on Gitea</a>.</p>"
                .to_owned(),
        }),
    };
    match data {
        TargetPage::Data(data) => {
            add_data(&mut res, &data)?;
            Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
        }
        TargetPage::Redirect(target) => Ok(HttpResponse::Found().insert_header((header::LOCATION, target)).finish()),
    }
}

#[route("/card/{id}", method = "GET", method = "HEAD")]
async fn card_info(card_id: web::Path<usize>) -> AnyResult<HttpResponse> {
    let mut res = String::with_capacity(2_000);
    let data = match CARDS_BY_ID.get(&card_id) {
        Some(card) => PageData {
            title:       format!("{} - {NAME}", card.name),
            description: card.short_info()?,
            query:       None,
            body:        format!(
                r#"<div> <img alt="Card Image: {}" class="fullimage" src="{}/static/full/{}.jpg"/>{card} <hr/> {} </div>"#,
                card.name,
                IMG_HOST.as_str(),
                card.id,
                card.extended_info().unwrap_or_else(|_| String::new()),
            ),
        },
        None => PageData {
            description: format!("Card not found - {NAME}"),
            title:       format!("Card not found - {NAME}"),
            query:       None,
            body:        "Card not found".to_owned(),
        },
    };
    add_data(&mut res, &data)?;
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}

#[route("/help", method = "GET", method = "HEAD")]
async fn help() -> AnyResult<HttpResponse> {
    let mut res = String::with_capacity(HEADER.len() + HELP_CONTENT.len() + 500);
    let data = PageData {
        query:       None,
        title:       format!("Query Syntax - {NAME}"),
        body:        HELP_CONTENT.to_owned(),
        description: String::new(),
    };
    add_data(&mut res, &data)?;
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}

fn add_searchbox(res: &mut String, query: &Option<String>) -> std::fmt::Result {
    write!(
        res,
        r#"
<form action="/">
  <input type="text" name="q" id="searchbox" placeholder="Enter query (e.g. l:5 c:synchro atk>2000)" value="{}"><input type="submit" id="submit" value="🔍">
</form>
"#,
        match &query {
            Some(q) => q.replace('"', "&quot;"),
            None => String::new(),
        }
    )
}

fn compute_results(raw_query: String) -> AnyResult<TargetPage> {
    let mut body = String::with_capacity(10_000);
    let (raw_filters, query) = match parser::parse_filters(raw_query.trim()) {
        Ok(q) => q,
        Err(e) => {
            let s = format!("Could not parse query: {e:?}");
            return Ok(TargetPage::Data(PageData {
                description: s.clone(),
                query:       Some(raw_query),
                body:        s,
                title:       NAME.to_owned(),
            }));
        }
    };
    let now = Instant::now();
    let matches: Vec<&Card> = SEARCH_CARDS
        .iter()
        .filter(|card| query.iter().all(|q| q(card)))
        .map(|c| CARDS_BY_ID.get(&c.id).unwrap())
        .take(RESULT_LIMIT)
        .collect();
    let readable_query = format!("Showing {} results where {}", matches.len(), raw_filters.iter().map(|f| f.to_string()).join(" and "),);
    write!(body, "<span class=\"meta\">{readable_query} (took {:?})</span>", now.elapsed())?;
    match matches[..] {
        [] => Ok(TargetPage::Data(PageData {
            description: readable_query,
            query: Some(raw_query),
            body,
            title: format!("No results - {NAME}"),
        })),
        [card] => Ok(TargetPage::Redirect(format!("/card/{}", card.id))),
        ref cards => {
            body.push_str("<div style=\"display: flex; flex-wrap: wrap;\">");
            for card in cards {
                write!(
                    body,
                    r#"<a class="cardresult" href="/card/{}"><img alt="Card Image: {}" src="{}/static/thumb/{}.jpg" class="thumb"/>{card}</a>"#,
                    card.id,
                    card.name,
                    IMG_HOST.as_str(),
                    card.id
                )?;
            }
            body.push_str("</div>");
            Ok(TargetPage::Data(PageData {
                description: readable_query,
                query: Some(raw_query),
                body,
                title: format!("{} results - {NAME}", cards.len()),
            }))
        }
    }
}

fn add_data(res: &mut String, pd: &PageData) -> AnyResult<()> {
    res.push_str(
        &HEADER.replacen("{DESCRIPTION}", &pd.description, 2).replacen("{IMG_HOST}", &IMG_HOST, 1).replacen("{TITLE}", &pd.title, 2),
    );
    add_searchbox(res, &pd.query)?;
    res.push_str(&pd.body);
    res.push_str(&footer());
    Ok(())
}
