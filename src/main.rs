use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;

#[derive(Debug, Clone)]
enum Source {
    File(PathBuf),
    Link(String),
}

impl FromStr for Source {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to parse as a path
        if let Ok(path) = PathBuf::from_str(s) {
            if path.is_file() {
                return Ok(Source::File(path));
            }
        }

        if url::Url::parse(s).is_ok() {
            return Ok(Source::Link(s.to_string()));
        }

        Err(format!("Invalid path or URL: {}", s))
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Cli {
    source: Source,
}

fn main() {
    let args = Cli::parse();
    println!("{:?}", args);

    match args.source {
        Source::File(path) => println!("File: {:?}", path),
        Source::Link(url) => println!("Link: {}", url),
    }
}
