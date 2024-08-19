use std::io::prelude::*;
use std::{fs::File, io::BufReader, path::PathBuf};
use tokio::task;

use serde::Deserialize;

use crate::deck::{Card, Deck};

pub async fn get_local_deck(path: PathBuf) -> anyhow::Result<Deck> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Collect lines into a Vec
    let mut lines = Vec::new();
    for line in reader.lines() {
        lines.push(line?);
    }

    // Create a vector of tasks
    let tasks: Vec<_> = lines
        .into_iter()
        .map(|line| {
            let line = line.clone();
            task::spawn(async move { parse_card(&line).await })
        })
        .collect();

    // Collect results from tasks
    let mut cards = Vec::new();
    let mut tokens = Vec::new();

    for task in tasks {
        let (parsed_cards, parsed_tokens) = task.await??;
        cards.extend(parsed_cards);
        tokens.extend(parsed_tokens);
    }

    Ok(Deck {
        name: "deck".to_string(),
        cards,
        tokens,
    })
}

#[derive(Debug, Deserialize)]
struct ScryfallCard {
    name: String,
    id: String,
    all_parts: Option<Vec<ScryfallRelatedCard>>,
}

#[derive(Debug, Deserialize)]
struct ScryfallRelatedCard {
    name: String,
    component: String,
    id: String,
}

// example card: 1 Whiptongue Hydra (NEC) 134
async fn parse_card(line: &str) -> anyhow::Result<(Vec<Card>, Vec<Card>)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let quantity = parts[0].parse::<u32>()?;
    let name = parts[1..parts.len() - 2].join(" ");
    let set = parts[parts.len() - 2].trim_matches(|c: char| !c.is_alphabetic());
    let collector_number = parts[parts.len() - 1];

    println!("{} {} {} {}", quantity, name, set, collector_number);

    let details = get_card_details(&name, set, collector_number).await?;

    let mut cards = Vec::new();
    let mut tokens = Vec::new();
    for _ in 0..quantity {
        let card = Card {
            name: details.name.clone(),
            scryfall_id: details.id.clone(),
        };

        if let Some(details) = &details.all_parts {
            for related in details {
                if related.component == "token" {
                    let token = Card {
                        name: related.name.clone(),
                        scryfall_id: related.id.clone(),
                    };
                    tokens.push(token);
                }
            }
        }

        cards.push(card);
    }

    Ok((cards, tokens))
}

async fn get_card_details(
    name: &str,
    set: &str,
    collector_number: &str,
) -> anyhow::Result<ScryfallCard> {
    let client = reqwest::Client::builder()
        .user_agent("curl/7.68.0")
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
            headers
        })
        .build()?;

    let resp = client
        .get("https://api.scryfall.com/cards/named")
        .query(&[
            ("exact", name),
            ("set", set),
            ("collector_number", collector_number),
        ])
        .send()
        .await?;

    let card: ScryfallCard = resp.json().await?;

    Ok(card)
}
