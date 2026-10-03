use crawler::url::URL;
use std::{env, vec::Vec};

use crawler::crawler::*;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: crawl <url>");
        std::process::exit(1);
    }
    let url = &args[1];
    let Ok(url) = URL::try_parse(url) else {
        println!("Invalid URL: {}", url);
        std::process::exit(1);
    };
    println!("Crawling {}", url);
    let mut already_crawled = Vec::<URL>::new();
    let urls = match crawl_page_recursive(&http_get, &url, &mut already_crawled) {
        Ok(urls) => urls,
        Err(error) => {
            println!("Error crawling page: {}", error);
            std::process::exit(1);
        }
    };
    println!("Found URLs on page {}:", url);
    println!(
        "{}",
        urls.iter()
            .map(|url| url.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
