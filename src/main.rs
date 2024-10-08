use clap::Parser;
use tiffer::deck::Deck;
use tiffer::local::get_local_deck;
use tiffer::remote::get_remote_deck;
use tiffer::source::Source;

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Cli {
    #[clap(short, long)]
    no_tokens: bool,
    #[clap(short, long)]
    filename: Option<String>,

    source: Source,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let deck: Deck = match args.source {
        Source::Link(url) => {
            println!("Fetching deck from remote: {}", url);
            get_remote_deck(url).await?
        }
        Source::File(path) => {
            println!("Deck from local: {}", path.to_str().unwrap());
            get_local_deck(path).await?
        }
    };

    deck.generate(tiffer::deck::DeckGenerationOptions {
        filename: args.filename,
        print_tokens: !args.no_tokens,
    })
    .await?;

    Ok(())
}
