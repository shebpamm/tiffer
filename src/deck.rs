extern crate printpdf;

use std::fs::{self, File};
use std::io::BufWriter;
use std::sync::Arc;

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;

use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use tokio::task;

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

    pub async fn download(&self, options: DeckGenerationOptions) -> anyhow::Result<()> {
        fs::create_dir_all("cards")?;

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

        for card in self.cards.iter().chain(self.tokens.iter()) {
            let client = Arc::clone(&client);
            let card = card.clone(); // Assuming `Card` implements `Clone`

            let task = task::spawn(async move {
                card.download(&client).await
            });

            tasks.push(task);
        }

        // Await all tasks
        for task in tasks {
            task.await??;
        }

        Ok(())
    }

    pub async fn generate(&self, options: DeckGenerationOptions) -> anyhow::Result<()> {
        println!("Generating deck: {}", self.name);
        println!(
            "Total cards: {} ({} mainboard, {} tokens)",
            self.total_cards(),
            self.cards.len(),
            self.tokens.len()
        );

        self.download(options).await?;

        self.pdf()?;

        Ok(())
    }

    fn pdf(&self) -> anyhow::Result<()> {
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
            let mut image_file =
                File::open(format!("{}/{}.jpg", "cards", card.scryfall_id)).unwrap();
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

        let mut buffer = BufWriter::new(File::create(format!("{}.pdf", self.name)).unwrap());
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

    pub async fn download(&self, client: &Client) -> anyhow::Result<()> {
        if std::fs::metadata(format!("{}/{}.jpg", "cards", self.scryfall_id)).is_ok() {
            println!("Skipping {}", self.name);
            return Ok(());
        }

        println!("Downloading {}", self.name);
        let response = client.get(self.image_url()).send().await?;
        let mut file = std::fs::File::create(format!("{}/{}.jpg", "cards", self.scryfall_id))?;
        std::io::copy(&mut response.bytes().await?.as_ref(), &mut file)?;
        Ok(())
    }
}
