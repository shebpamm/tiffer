use clap::Parser;
use tiffer::deck::Deck;
use tiffer::remote::get_remote_deck;
use tiffer::source::Source;

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Cli {
    source: Source,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let deck: Deck = match args.source {
        Source::Link(url) => {
            println!("Fetching deck from remote: {}", url);
            get_remote_deck(url)?
        }
        Source::File(path) => {
            println!("Deck from local: {}", path.to_str().unwrap());
            todo!()
        }
    };

    deck.download()?;

    Ok(())
}
