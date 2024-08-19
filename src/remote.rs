use std::collections::HashMap;

use derive_more::Display;
use serde::Deserialize;
use thiserror::Error;

use url::Url;

use crate::deck::Card;
use crate::deck::Deck;

const MOXFIELD_API_URL: &str = "https://api2.moxfield.com/v2/decks/all";

#[derive(Debug, Display, Error)]
enum DeckDownloadError {
    UnsupportedWebsite,
}

enum Website {
    Moxfield,
}

pub async fn get_remote_deck(url: Url) -> anyhow::Result<Deck> {
    let website = parse_url(&url)?;
    match website {
        Website::Moxfield => Ok(get_moxfield_deck(url).await?),
    }
}

fn parse_url(url: &Url) -> anyhow::Result<Website> {
    match url.host_str() {
        Some("moxfield.com") => Ok(Website::Moxfield),
        _ => Err(DeckDownloadError::UnsupportedWebsite.into()),
    }
}

async fn get_moxfield_deck(url: Url) -> anyhow::Result<Deck> {
    #[derive(Debug, Deserialize)]
    struct MoxfieldCard {
        pub quantity: u32,
        pub card: MoxfieldCardInfo,
    }

    #[derive(Debug, Deserialize)]
    struct MoxfieldCardInfo {
        pub scryfall_id: String,
        pub name: String,
    }

    #[derive(Debug, Deserialize)]
    struct MoxfieldResponse {
        pub name: String,
        pub main: MoxfieldCardInfo,
        pub mainboard: HashMap<String, MoxfieldCard>,
        pub tokens: Vec<MoxfieldCardInfo>,
    }

    let deck_id = url.path_segments().unwrap().nth(1).unwrap();
    let deck_url = format!("{}/{}", MOXFIELD_API_URL, deck_id);
    let response = reqwest::get(deck_url).await?;

    // pretty-print json for easier debugging
    let data = serde_json::to_string_pretty(&response.json::<serde_json::Value>().await?)?;

    let response: MoxfieldResponse = serde_json::from_str(&data)?;

    let mut cards = Vec::new();

    cards.push(Card {
        name: response.main.name,
        scryfall_id: response.main.scryfall_id,
    });
    for card in response.mainboard.values() {
        for _ in 0..card.quantity {
            cards.push(Card {
                name: card.card.name.clone(),
                scryfall_id: card.card.scryfall_id.clone(),
            });
        }
    }

    let mut tokens = Vec::new();
    for card in response.tokens {
        tokens.push(Card {
            name: card.name.clone(),
            scryfall_id: card.scryfall_id.clone(),
        });
    }

    Ok(Deck {
        name: response.name,
        cards,
        tokens,
    })
}
