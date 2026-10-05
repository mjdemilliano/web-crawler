use crawler::url::URL;
use std::{env, vec::Vec};

use crawler::crawler::*;

struct Args {
    /// URL to crawl.
    pub url: URL,
    /// Maximum number of URLs to crawl.
    pub limit: Option<usize>,
}

impl Args {
    fn show_help_and_exit() {
        eprintln!("Usage: crawl [--limit N] <url>");
        std::process::exit(1);
    }

    fn parse(mut args: Vec<String>) -> Result<Self, String> {
        // Pop program name.
        args.remove(0);
        let mut limit: Option<usize> = None;
        while args.len() > 1 {
            match args[0].as_ref() {
                "--limit" => {
                    limit = match args[1].parse::<usize>() {
                        Ok(limit) => Some(limit),
                        Err(_) => {
                            return Err(format!("Must be a number: {}", args[1]));
                        }
                    };
                    args.remove(0);
                    args.remove(0);
                }
                "--help" => {
                    Args::show_help_and_exit();
                }
                _ => {}
            }
        }
        if args.len() != 1 {
            return Err(String::from("missing required argument: url"));
        }
        let url = &args[0];
        let Ok(url) = URL::try_parse(url) else {
            return Err(format!("Invalid URL: {}", url));
        };
        Ok(Args { url, limit })
    }
}

fn main() {
    let args = match Args::parse(env::args().collect()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{}", message);
            std::process::exit(1);
        }
    };
    println!("Crawling {}", args.url);
    let urls = match crawl_page_recursive(&http_get, &args.url, args.limit) {
        Ok(urls) => urls,
        Err(error) => {
            println!("Error crawling page: {}", error);
            std::process::exit(1);
        }
    };
    println!("Found URLs on page {}:", args.url);
    println!(
        "{}",
        urls.iter()
            .map(|url| url.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
