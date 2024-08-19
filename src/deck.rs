extern crate printpdf;

use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::sync::Arc;
use std::time::Duration;

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;

use reqwest::header::{HeaderValue, RETRY_AFTER};
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use tokio::task;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeckGenerationOptions {
    pub print_tokens: bool,
    pub filename: Option<String>,
}

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

    fn cache_dir(&self) -> String {
        format!(
            "{}/tiffer/cards",
            std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
                format!(
                    "{}/.cache",
                    std::env::var("HOME").expect("HOME environment variable not set")
                )
            })
        )
    }

    pub async fn download(&self, options: DeckGenerationOptions) -> anyhow::Result<()> {
        let cache_dir = self.cache_dir();
        fs::create_dir_all(&cache_dir)?;

        // Create reqwest client
        let client = reqwest::Client::builder()
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

        // Wrap the client in an Arc to share between tasks
        let client = Arc::new(client);

        // Create a vector of tasks
        let mut tasks = Vec::new();

        let mut cards = Vec::new();
        cards.extend(self.cards.iter().cloned()); // assuming `Card` implements `Clone`
        if options.print_tokens {
            cards.extend(self.tokens.iter().cloned()); // assuming `Token` implements `Clone`
        }

        for card in cards {
            let client = Arc::clone(&client);
            let card = card.clone(); // Assuming `Card` implements `Clone`
            let cache_dir = cache_dir.clone();

            let task = task::spawn(async move { card.download(cache_dir.clone(), &client).await });

            tasks.push(task);
        }

        // Await all tasks
        for task in tasks {
            task.await??;
        }

        Ok(())
    }

    pub async fn generate(&self, options: DeckGenerationOptions) -> anyhow::Result<()> {
        println!("Generating deck: {}", &options.filename.clone().unwrap_or_else(|| self.name.clone()));
        match options.print_tokens {
            true => println!(
                "Total cards: {} ({} mainboard, {} tokens)",
                self.total_cards(),
                self.cards.len(),
                self.tokens.len()
            ),
            false => {
                println!("Skipping tokens...");
                println!(
                    "Total cards: {} ({} mainboard)",
                    self.total_cards(),
                    self.cards.len()
                )
            }
        }

        self.download(options.clone()).await?;

        self.pdf(options)?;

        Ok(())
    }

    fn pdf(&self, options: DeckGenerationOptions) -> anyhow::Result<()> {
        let cache_dir = self.cache_dir();

        let (doc, page_idx, layer_idx) = PdfDocument::new("Deck", Mm(210.0), Mm(297.0), "Layer");

        let (card_width, card_height) = (Mm(63.0), Mm(87.8));

        let num_cards_in_row = 3;
        let total_row_width = Mm(num_cards_in_row as f32 * card_width.0);
        let mut x = (Mm(210.0) - total_row_width) / 2.0;
        let mut y = Mm(297.0) - card_height - Mm(15.0);

        let mut layer = doc.get_page(page_idx).get_layer(layer_idx);

        // reference size box for card
        let points = vec![
            (Point::new(Mm(0.0), Mm(0.0)), false),
            (Point::new(card_width, Mm(0.0)), false),
            (Point::new(card_width, card_height), false),
            (Point::new(Mm(0.0), card_height), false),
        ];
        let _line = Polygon {
            rings: vec![points],
            mode: PaintMode::FillStroke,
            winding_order: WindingOrder::NonZero,
        };

        // layer.add_polygon(line);

        for card in self.cards.iter().chain(self.tokens.iter()) {
            println!("Rendering {}", card.name);
            let mut image_file = BufReader::new(
                File::open(format!("{}/{}.jpg", cache_dir, card.scryfall_id)).unwrap(),
            );
            let image = Image::try_from(
                image_crate::codecs::jpeg::JpegDecoder::new(&mut image_file).unwrap(),
            )
            .unwrap();

            if x + card_width > Mm(210.0) {
                x = (Mm(210.0) - total_row_width) / 2.0;
                y -= card_height;
            }

            if y < Mm(0.0) {
                let (new_page_idx, _) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                layer = doc.get_page(new_page_idx).get_layer(layer_idx);
                y = Mm(297.0) - card_height - Mm(15.0);
            }

            image.add_to_layer(
                layer.clone(),
                ImageTransform {
                    translate_x: Some(x),
                    translate_y: Some(y),
                    dpi: Some(1200.0),
                    scale_x: Some(4.42),
                    scale_y: Some(4.42),
                    rotate: Some(ImageRotation::default()),
                },
            );

            x += card_width;
        }

        let filename = options.filename.clone().unwrap_or_else(|| format!("{}.pdf", self.name));
        let mut buffer = BufWriter::new(File::create(filename).unwrap());
        doc.save(&mut buffer).unwrap();

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

    pub async fn download(&self, cache: String, client: &Client) -> anyhow::Result<()> {
        let file_path = format!("{}/{}.jpg", cache, self.scryfall_id);
        if fs::metadata(&file_path).is_ok() {
            println!("Skipping {}", self.name);
            return Ok(());
        }

        println!("Downloading {}", self.name);

        const MAX_RETRIES: usize = 5;

        let mut attempts = 0;

        while attempts < MAX_RETRIES {
            attempts += 1;
            let response = client.get(self.image_url()).send().await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let mut file = fs::File::create(&file_path)?;
                        std::io::copy(&mut resp.bytes().await?.as_ref(), &mut file)?;
                        println!("Successfully downloaded {}", self.name);
                        return Ok(());
                    } else if resp.status().as_u16() == 429 {
                        // Rate limit exceeded, retry after backoff
                        println!("Rate limit exceeded, retrying {}...", self.name);
                        if let Some(retry_after) = resp.headers().get(RETRY_AFTER) {
                            let delay = parse_retry_after_header(retry_after)?;
                            sleep(Duration::from_secs(delay)).await;
                        } else {
                            // Default backoff if Retry-After header is missing
                            let delay = 2u64.pow(attempts as u32);
                            sleep(Duration::from_secs(delay)).await;
                        }
                    } else {
                        // Other errors, return the result
                        return Err(anyhow::anyhow!(
                            "Failed to download {}: {}",
                            self.name,
                            resp.status()
                        ));
                    }
                }
                Err(e) => {
                    // Handle request error
                    println!("Error occurred while downloading {}: {}", self.name, e);
                    if attempts >= MAX_RETRIES {
                        return Err(anyhow::anyhow!(
                            "Failed to download {} after {} attempts",
                            self.name,
                            MAX_RETRIES
                        ));
                    }
                    // Default backoff if request fails
                    let delay = 2u64.pow(attempts as u32);
                    sleep(Duration::from_secs(delay)).await;
                }
            }
        }

        Err(anyhow::anyhow!(
            "Failed to download {} after {} attempts",
            self.name,
            MAX_RETRIES
        ))
    }
}

// Helper function to parse Retry-After header
fn parse_retry_after_header(header: &HeaderValue) -> anyhow::Result<u64> {
    if let Ok(s) = header.to_str() {
        if let Ok(seconds) = s.parse::<u64>() {
            return Ok(seconds);
        }
        // Retry-After header is a date-time
        // Parse the date-time format if needed
    }
    // Return a default value if parsing fails
    Ok(60) // Default to 60 seconds
}
