use std::io::{self, prelude::*};
use std::{fs::File, io::BufReader, path::PathBuf};

use crate::deck::{Card, Deck};

pub fn get_local_deck(path: PathBuf) -> anyhow::Result<Deck> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut cards: Vec<Card> = Vec::new();

    for line in reader.lines() {
        let line = line?;

        let parsed = parse_card(&line)?;

        for card in parsed {
            cards.push(card);
        }
    }

    Ok(Deck {
        name: "deck".to_string(),
        cards,
        tokens: Vec::new(),
    })
}

// example card: 1 Whiptongue Hydra (NEC) 134
fn parse_card(line: &str) -> anyhow::Result<Vec<Card>> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let quantity = parts[0].parse::<u32>()?;
    let name = parts[1..parts.len() - 2].join(" ");
    let set = parts[parts.len() - 2].trim_matches(|c: char| !c.is_alphabetic());
    let collector_number = parts[parts.len() - 1];

    println!("{} {} {} {}", quantity, name, set, collector_number);

    let scryfall_id = get_card_uuid(&name, set, collector_number)?;

    let mut cards = Vec::new();
    for _ in 0..quantity {
        let card = Card {
            name: name.clone(),
            scryfall_id: scryfall_id.clone(),
        };

        cards.push(card);
    }

    Ok(cards)
}

fn get_card_uuid(name: &str, set: &str, collector_number: &str) -> anyhow::Result<String> {
    #[derive(Debug, serde::Deserialize)]
    struct Response {
        pub id: String,
    }

    let client = reqwest::blocking::Client::builder()
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
        .query(&[("exact", name), ("set", set), ("collector_number", collector_number)])
        .send()?;

    let Response { id } = resp.json()?;

    Ok(id)
}
