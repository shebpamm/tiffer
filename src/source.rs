use std::path::PathBuf;
use std::str::FromStr;

use url::Url;

#[derive(Debug, Clone)]
pub enum Source {
    File(PathBuf),
    Link(Url),
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

        if let Ok(url) = Url::parse(s) {
            return Ok(Source::Link(url));
        }

        Err(format!("Invalid path or URL: {}", s))
    }
}
