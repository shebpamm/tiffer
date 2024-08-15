use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct Deck {
    pub name: String,
    pub cards: Vec<Card>,
    pub tokens: Vec<Card>,
}

impl Deck {
    pub fn total_cards(&self) -> usize {
        self.cards.len() + self.tokens.len()
    }

    pub fn download(&self) -> anyhow::Result<()> {
        println!("Downloading deck: {}", self.name);
        println!("Total cards: {} ({} mainboard, {} tokens)", self.total_cards(), self.cards.len(), self.tokens.len());
        std::fs::create_dir_all("cards")?;

        // make sure to add User-Agent and Accept headers
        let client = reqwest::blocking::Client::builder()
            .user_agent("curl/7.68.0")
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::ACCEPT,
                    reqwest::header::HeaderValue::from_static("image/jpeg"),
                );
                headers
            })
            .build()?;

        for card in self.cards.iter().chain(self.tokens.iter()) {
            if std::fs::metadata(format!("{}/{}.jpg", "cards", card.scryfall_id)).is_ok() {
                println!("Skipping {}", card.name);
                continue;
            }

            println!("Downloading {}", card.name);
            let response = client.get(&card.image_url()).send()?;
            let mut file = std::fs::File::create(format!("{}/{}.jpg", "cards", card.scryfall_id))?;
            std::io::copy(&mut response.bytes().unwrap().as_ref(), &mut file)?;
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Card {
    pub name: String,
    pub scryfall_id: String,
}

impl Card {
    pub fn image_url(&self) -> String {
        format!(
            "https://api.scryfall.com/cards/{}/?format=image",
            self.scryfall_id
        )
    }
}
