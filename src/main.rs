#![feature(test, trim_prefix_suffix)]
extern crate test;
use actix_web::{http::header, route, web, App, HttpResponse, HttpServer};
use data::{Card, CardInfo, Set};
use filter::SearchCard;
use itertools::Itertools;
use parser::Field;
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

// Not 100 because many modern sets have exactly 101 cards (100 + 1 bonus like the 25th anniversary celebrations).
// I want all of those to fit on one page.
const PAGE_SIZE: usize = 120;

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

// Matches a quoted archetype/card name followed by an optional type qualifier.
// Group 1: name inside quotes.
// Group 2: typeline including leading space (e.g. " Spell Card", " Synchro Monster").
// Group 3: only main card type: Spell/Trap | Spell | Trap | Monster.
// Group 4: Single trailing character to exclude false positives (see usage site).
// I’d use lookahead, but the regex crate doesn’t support it.
static QUOTED_TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""([^"<>\n]+)"(\s+(?:[A-Z][a-zA-Z-]*\s+)*(Spells?/Traps?|Spells?|Traps?|[Mm]onsters?|cards?)(?:\s+[Cc]ard)?)?(.?)"#)
        .unwrap()
});

// Matches TYPE that mentions "Name", e.g. "Equip Spell that mentions "Adventurer Token"".
// Applied before QUOTED_TERM. Uses &quot;/single-quoted attrs in output to prevent re-matching.
static MENTIONS_TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?P<qualifier>(?:[A-Za-z][a-zA-Z/-]*\s+)*(?P<type_keyword>Spells?/Traps?|Spells?|Traps?|[Mm]onsters?|[Cc]ards?))\s+that mentions\s+"(?P<name>[^"<>\n]+)""#,
    )
    .unwrap()
});

// Matches "Name", or [N] TYPE that mentions it, e.g. '"Invocation", or 1 Spell that mentions it'.
// Applied before MENTIONS_TERM and QUOTED_TERM.
static MENTIONS_IT_TERM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#""(?P<name>[^"<>\n]+)"(?P<between>,?\s+or\s+\d*\s*)(?P<qualifier>(?:[A-Za-z][a-zA-Z/-]*\s+)*(?P<type_keyword>Spells?/Traps?|Spells?|Traps?|[Mm]onsters?|[Cc]ards?))\s+that mentions it"#,
    )
    .unwrap()
});

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
    p: Option<usize>,
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
<span class="viewcount">{}</span>
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
async fn search(q: Option<web::Query<Query>>) -> AnyResult<HttpResponse> {
    let mut res = String::with_capacity(10_000);
    let data = match q {
        Some(web::Query(Query { q, p })) if !q.is_empty() => compute_results(q, p.unwrap_or(0))?,
        _ => TargetPage::Data(PageData {
            title:       NAME.to_owned(),
            description: "Enter a query above to search".to_owned(),
            query:       None,
            body:        "<p>Welcome to my cheap Scryfall clone for Yugioh.</p>\
                          <p>Enter a query above to search or read the <a href=\"/help\">query syntax</a> for more information.</p>\
                          <p>The source code is available <a href=\"https://github.com/kageru/aro\">on Github</a>.</p>\
                          <p>If you have any feedback, feel free to add @kageru on Discord or send an email to &lt;that name&gt;@encode.moe.</p>"
                .to_owned(),
        }),
    };
    match data {
        TargetPage::Data(data) => {
            add_data(&mut res, &data, None)?;
            Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
        }
        TargetPage::Redirect(target) => Ok(HttpResponse::Found().insert_header((header::LOCATION, target)).finish()),
    }
}

#[route("/card/{id}", method = "GET", method = "HEAD")]
async fn card_info(card_id: web::Path<usize>) -> AnyResult<HttpResponse> {
    let mut res = String::with_capacity(2_000);
    let data = match CARDS_BY_ID.get(&card_id) {
        Some(card) => {
            let card = Card { text: add_search_links(&card.text, &card.name), ..card.clone() };
            PageData {
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
            }
        }
        None => PageData {
            description: format!("Card not found - {NAME}"),
            title:       format!("Card not found - {NAME}"),
            query:       None,
            body:        "Card not found".to_owned(),
        },
    };
    add_data(&mut res, &data, Some(*card_id))?;
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
    add_data(&mut res, &data, None)?;
    Ok(HttpResponse::Ok().insert_header(header::ContentType::html()).body(res))
}

// Builds the query string for a "mentions" link with type filters and o:"name".
fn build_mentions_query(qualifier: &str, type_keyword: &str, name_query: &str) -> String {
    let mut filters: Vec<String> = qualifier
        .split_whitespace()
        .filter(|&w| !w.eq_ignore_ascii_case(type_keyword) && !w.eq_ignore_ascii_case("card") && !w.eq_ignore_ascii_case("cards"))
        .map(|w| format!("t:{}", w.to_lowercase()))
        .collect();
    let type_filter = match type_keyword.to_lowercase().trim_suffix("s") {
        "monster" => "t:monster",
        "spell/trap" | "spells/traps" => "t:spell|trap",
        "spell" => "t:spell",
        "trap" => "t:trap",
        _ => "",
    };
    if !type_filter.is_empty() {
        filters.push(type_filter.to_owned());
    }
    // %22 is " url encoded
    filters.push(format!("o:%22{name_query}%22"));
    filters.join("+")
}

fn add_search_links(text: &str, card_name: &str) -> String {
    let text = MENTIONS_IT_TERM.replace_all(text, |caps: &Captures| {
        let qualifier    = caps.name("qualifier").unwrap().as_str();
        let type_keyword = caps.name("type_keyword").unwrap().as_str();
        let name         = caps.name("name").unwrap().as_str();
        let between      = caps.name("between").unwrap().as_str();
        let query_name = name.to_lowercase().replace(' ', "+");
        let type_query = build_mentions_query(qualifier, type_keyword, &query_name);
        format!(r#"<a href='/?q={query_name}' class='cardlink'>&quot;{name}&quot;</a>{between}<a href='/?q={type_query}' class='cardlink'>{qualifier} that mentions it</a>"#)
    });
    let text = MENTIONS_TERM.replace_all(&text, |caps: &Captures| {
        let qualifier = caps.name("qualifier").unwrap().as_str();
        let type_keyword = caps.name("type_keyword").unwrap().as_str();
        let name = caps.name("name").unwrap().as_str();
        let query = build_mentions_query(qualifier, type_keyword, &name.to_lowercase().replace(' ', "+"));
        format!(r#"<a href='/?q={query}' class='cardlink'>{qualifier} that mentions &quot;{name}&quot;</a>"#)
    });
    let link_to_self = card_name.to_lowercase().replace(' ', "+");
    QUOTED_TERM
        .replace_all(&text, |caps: &Captures| {
            // Group 4 is the character immediately following the match.
            // If it's a quote or alphanumeric the match is a false positive caused by a card
            // name that itself contains quotes (e.g. K9 "Jacks"), so return it unchanged.
            let trailing = caps.get(4).map_or("", |m| m.as_str());
            if trailing.chars().next().is_some_and(|c| c == '"' || c.is_alphanumeric()) {
                return caps[0].to_string();
            }
            let name = &caps[1];
            let query_name = name.to_lowercase().replace(' ', "+");
            if query_name == link_to_self {
                return format!(r#""{name}"{trailing}"#);
            }
            let suffix = caps.get(2).map_or("", |m| m.as_str());
            let type_keyword = caps.get(3).map_or("", |m| m.as_str());
            let subtype_filters: String = suffix
                .split_whitespace()
                .filter(|&w| w != type_keyword && !w.eq_ignore_ascii_case("card"))
                .map(|w| format!("+t:{}", w.to_lowercase()))
                .collect();
            let type_filter = match type_keyword {
                "Monster" | "monster" => "+t:monster",
                "Spell/Trap" => "+t:spell|trap",
                "Spell" => "+t:spell",
                "Trap" => "+t:trap",
                _ => "",
            };
            format!(r#"<a href="/?q={query_name}{subtype_filters}{type_filter}" class="cardlink">"{name}"{suffix}</a>{trailing}"#)
        })
        .into_owned()
}

fn add_searchbox(res: &mut String, query: &Option<String>) -> std::fmt::Result {
    write!(
        res,
        r#"
<form action="/">
  <input type="text" name="q" autofocus id="searchbox" placeholder="Enter query (e.g. l:5 c:synchro atk>2000)" value="{}"><input type="submit" id="submit" value="🔍">
</form>
"#,
        match &query {
            Some(q) => q.replace('"', "&quot;"),
            None => String::new(),
        }
    )
}

fn compute_results(raw_query: String, page: usize) -> AnyResult<TargetPage> {
    let mut body = String::with_capacity(10_000);
    let (raw_filters, query) = match parser::parse_filters(raw_query.trim()) {
        Ok((raw, mut parsed)) => {
            if raw.iter().any(|r| r.0 == Field::Genesys) {
                parsed.push(Box::new(SearchCard::genesys_legal));
            }
            (raw, parsed)
        }
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
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
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
        // Don’t want the `>>` button to redirect to a single card view, even if there is only one result left.
        [card] if page == 0 => Ok(TargetPage::Redirect(format!("/card/{}", card.id))),
        ref cards => {
            body.push_str("<div style=\"display: flex; flex-wrap: wrap;\">");
            for card in cards {
                write!(
                    body,
                    r#"<a class="cardresult hoverable" href="/card/{}"><img alt="Card Image: {}" src="{}/static/thumb/{}.jpg" class="thumb"/>{card}</a>"#,
                    card.id,
                    card.name,
                    IMG_HOST.as_str(),
                    card.id
                )?;
            }
            body.push_str("</div>");
            // It’s possible that we’ve exactly reached the end of the results and the next page is empty.
            // No simple fix comes to mind. Maybe take() 1 result more than we show and check that way?
            let has_next = cards.len() == PAGE_SIZE;
            let has_prev = page > 0;
            let url_query = urlencoding::encode(&raw_query);
            if has_next || has_prev {
                body.push_str("<p style=\"font-size: 160%; display: flex;\">");
                if has_prev {
                    write!(body, "<a class=\"hoverable pagearrow\" href=\"/?q={url_query}&p={}\">&lt;&lt;</a>", page.saturating_sub(1))?;
                }
                if has_next {
                    write!(body, "<a class=\"hoverable pagearrow\" href=\"/?q={url_query}&p={}\">&gt;&gt;</a>", page + 1)?;
                }
                body.push_str("</p>");
            }
            Ok(TargetPage::Data(PageData {
                description: readable_query,
                query: Some(raw_query),
                body,
                title: format!("{} results - {NAME}", cards.len()),
            }))
        }
    }
}

fn add_data(res: &mut String, pd: &PageData, card_id: Option<usize>) -> AnyResult<()> {
    res.push_str(
        &HEADER
            .replacen("{DESCRIPTION}", &pd.description.replace('"', r#"\""#), 2)
            .replacen("{IMG_HOST}", &IMG_HOST, 2)
            .replacen("{TITLE}", &pd.title, 2)
            .replacen(
                "{OG_IMAGE}",
                &match card_id {
                    Some(id) => format!(r#"<meta property="og:image" content="{}/static/full/{id}.jpg" />"#, IMG_HOST.as_str()),
                    None => String::new(),
                },
                1,
            ),
    );
    add_searchbox(res, &pd.query)?;
    res.push_str(&pd.body);
    res.push_str(&footer());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // You may notice that the card name in most of these doesn’t match the effect.
    // That’s because I’m lazy and let Claude generate my test code,
    // which then hallucinates a Branded Beast that can fusion summon.
    // I’m keeping these because, again, I’m lazy.
    #[test]
    fn quoted_term_linkification() {
        // "card" included in link text, no type filter
        assert_eq!(
            add_search_links(r#"Add 1 "Swordsoul" card from your Deck to your hand."#, "Swordsoul Strategist Longyuan"),
            r#"Add 1 <a href="/?q=swordsoul" class="cardlink">"Swordsoul" card</a> from your Deck to your hand."#
        );
        // Lowercase monster qualifier
        assert_eq!(
            add_search_links(r#"Special Summon 1 "Swordsoul" monster from your Deck."#, "Swordsoul Strategist Longyuan"),
            r#"Special Summon 1 <a href="/?q=swordsoul+t:monster" class="cardlink">"Swordsoul" monster</a> from your Deck."#
        );
        // Uppercase adjective + uppercase Monster (specific extra deck type)
        assert_eq!(
            add_search_links(r#"Fusion Summon 1 "Branded" Fusion Monster."#, "Branded Beast"),
            r#"Fusion Summon 1 <a href="/?q=branded+t:fusion+t:monster" class="cardlink">"Branded" Fusion Monster</a>."#
        );
        // Uppercase adjective + lowercase monster
        assert_eq!(
            add_search_links(r#"Fusion Summon 1 "Branded" Fusion monster."#, "Branded Beast"),
            r#"Fusion Summon 1 <a href="/?q=branded+t:fusion+t:monster" class="cardlink">"Branded" Fusion monster</a>."#
        );
        // Spell Card
        assert_eq!(
            add_search_links(r#"Add 1 "Branded" Spell Card from your Deck."#, "Branded Beast"),
            r#"Add 1 <a href="/?q=branded+t:spell" class="cardlink">"Branded" Spell Card</a> from your Deck."#
        );
        // Subtype before Spell
        assert_eq!(
            add_search_links(r#"Add 1 "K9" Quick-Play Spell from your Deck."#, "K9-66X \"Jacks\""),
            r#"Add 1 <a href="/?q=k9+t:quick-play+t:spell" class="cardlink">"K9" Quick-Play Spell</a> from your Deck."#
        );
        // Spell/Trap
        assert_eq!(
            add_search_links(r#"Set 1 "Branded" Spell/Trap from your Deck."#, "Branded Beast"),
            r#"Set 1 <a href="/?q=branded+t:spell|trap" class="cardlink">"Branded" Spell/Trap</a> from your Deck."#
        );
        // Multi-word card name, no type qualifier
        assert_eq!(
            add_search_links(r#"Tribute "Blue-Eyes White Dragon"."#, "Kaibaman"),
            r#"Tribute <a href="/?q=blue-eyes+white+dragon" class="cardlink">"Blue-Eyes White Dragon"</a>."#
        );
        // Self-reference — must not be linkified
        assert_eq!(
            add_search_links(
                r#"You can only use each effect of "Swordsoul Strategist Longyuan" once per turn."#,
                "Swordsoul Strategist Longyuan"
            ),
            r#"You can only use each effect of "Swordsoul Strategist Longyuan" once per turn."#
        );
        // Card name containing quotes — neither fragment must be linkified
        assert_eq!(
            add_search_links(r#"You can only use this effect of "K9-66X "Jacks"" once per turn."#, r#"K9-66X "Jacks""#),
            r#"You can only use this effect of "K9-66X "Jacks"" once per turn."#
        );
        // "mentions" — single-type qualifier
        assert_eq!(
            add_search_links(r#"add 1 monster that mentions "Clear World" from your Deck to your hand."#, "Clear World Guard"),
            r#"add 1 <a href='/?q=t:monster+o:%22clear+world%22' class='cardlink'>monster that mentions &quot;Clear World&quot;</a> from your Deck to your hand."#
        );
        // "mentions" — two-word qualifier with subtype
        assert_eq!(
            add_search_links(
                r#"Add 1 Equip Spell that mentions "Adventurer Token" from your Deck to your hand."#,
                "Water Enchantress of the Temple"
            ),
            r#"Add 1 <a href='/?q=t:equip+t:spell+o:%22adventurer+token%22' class='cardlink'>Equip Spell that mentions &quot;Adventurer Token&quot;</a> from your Deck to your hand."#
        );
        // "mentions" — Fusion Monster qualifier
        assert_eq!(
            add_search_links(r#"Special Summon 1 Fusion Monster that mentions "Fallen of Albaz"."#, "Mirrorjade the Iceblade Dragon"),
            r#"Special Summon 1 <a href='/?q=t:fusion+t:monster+o:%22fallen+of+albaz%22' class='cardlink'>Fusion Monster that mentions &quot;Fallen of Albaz&quot;</a>."#
        );
        // "mentions it" — name link + type+o: link
        assert_eq!(
            add_search_links(r#"Add 1 "Invocation", or 1 Spell that mentions it, from your Deck to your hand."#, "Aleister the Invoker"),
            r#"Add 1 <a href='/?q=invocation' class='cardlink'>&quot;Invocation&quot;</a>, or 1 <a href='/?q=t:spell+o:%22invocation%22' class='cardlink'>Spell that mentions it</a>, from your Deck to your hand."#
        );
        // "mentions it" — generic "card" type keyword, no comma before "or"
        assert_eq!(
            add_search_links(
                r#"You can send 1 "Fallen of Albaz" or 1 card that mentions it from your Deck to the GY."#,
                "Bystial Magnamhut"
            ),
            r#"You can send 1 <a href='/?q=fallen+of+albaz' class='cardlink'>&quot;Fallen of Albaz&quot;</a> or 1 <a href='/?q=o:%22fallen+of+albaz%22' class='cardlink'>card that mentions it</a> from your Deck to the GY."#
        );
    }
}

#[cfg(all(test, not(debug_assertions)))]
mod bench {
    use super::*;
    use std::hint::black_box;

    #[bench]
    fn search_result_bench(b: &mut test::Bencher) {
        let query = "test".to_owned();
        assert!(compute_results(query.clone(), 0).is_ok());
        b.iter(|| assert!(compute_results(black_box(query.clone()), 0).is_ok()))
    }
}
