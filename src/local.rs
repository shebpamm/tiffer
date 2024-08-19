use std::io::prelude::*;
use std::time::Duration;
use std::{fs::File, io::BufReader, path::PathBuf};
use reqwest::header::HeaderMap;
use tokio::task;

use serde::Deserialize;
use tokio::time::sleep;

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

    let details = match get_card_details(&name, set, collector_number).await {
        Ok(details) => details,
        Err(e) => {
            eprintln!("Failed to get card details for {}: {}", name, e);
            return Err(e);
        }
    };

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
    const MAX_RETRIES: u32 = 5;
    const INITIAL_BACKOFF_SECS: u64 = 1;

    let client = reqwest::Client::builder()
        .user_agent("curl/7.68.0")
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
            headers
        })
        .build()?;

    let mut attempt = 0;

    while attempt < MAX_RETRIES {
        let resp = client
            .get("https://api.scryfall.com/cards/named")
            .query(&[
                ("exact", name),
                ("set", set),
                ("collector_number", collector_number),
            ])
            .send()
            .await?;

        if resp.status().is_success() {
            let card: ScryfallCard = resp
                .json()
                .await?;
            return Ok(card);
        } else if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // Extract the retry-after duration from the headers if available
            if let Some(retry_after_header) = resp.headers().get("Retry-After") {
                if let Ok(retry_after) = retry_after_header.to_str().unwrap().parse::<u64>() {
                    let backoff_duration = Duration::from_secs(retry_after);
                    eprintln!("Rate limit exceeded. Retrying after {} seconds...", retry_after);
                    sleep(backoff_duration).await;
                } else {
                    eprintln!("Rate limit exceeded but no valid Retry-After header provided.");
                    // Use a default backoff duration if Retry-After header is invalid
                    sleep(Duration::from_secs(INITIAL_BACKOFF_SECS)).await;
                }
            } else {
                eprintln!("Rate limit exceeded but no Retry-After header found. Using default backoff.");
                sleep(Duration::from_secs(INITIAL_BACKOFF_SECS)).await;
            }
        } else {
            return Err(anyhow::anyhow!("Failed to get card details: {}", resp.status()));
        }

        attempt += 1;
    }

    Err(anyhow::anyhow!("Exceeded maximum number of retries"))
}
