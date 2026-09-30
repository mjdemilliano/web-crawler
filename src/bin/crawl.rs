use scraper::{Html, Selector};
use std::{env, fmt::Display, vec::Vec};
use ureq;

use crate::Error::HTTPError;

type URLRef<'a> = &'a str;
type URL = String;

#[derive(Debug)]
enum Error {
    HTTPError(ureq::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HTTPError(error) => write!(f, "HTTP error: {:?}", error),
        }
    }
}

fn crawl_page<G>(page_getter: G, url: URLRef) -> Result<Vec<URL>, Error>
where
    G: Fn(URLRef) -> Result<String, Error>,
{
    let contents = page_getter(url)?;
    let fragment = Html::parse_fragment(&contents);
    let a_selector = Selector::parse("a").unwrap();
    let urls = fragment
        .select(&a_selector)
        .filter_map(|element| element.value().attr("href"))
        .map(|href| String::from(href))
        .collect();
    Ok(urls)
}

fn http_get(url: URLRef) -> Result<String, Error> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| Error::HTTPError(error))?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|error| Error::HTTPError(error))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: crawl <url>");
        std::process::exit(1);
    }
    let url = &args[1];
    println!("Crawling {}", url);
    let urls = match crawl_page(http_get, &url) {
        Ok(urls) => urls,
        Err(error) => {
            println!("Error crawling page: {}", error);
            std::process::exit(1);
        }
    };
    println!("Found URLs on page {}:", url);
    println!("{}", urls.join("\n"));
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_crawl_page() {
        let data = "blabla<a href='foo'>burp</a><a href='/foo'>barf</a>
            <p>Blabla</p><a href='http://some.domain.com/oeps/bla.html'>Some link</a>.";
        let result =
            crawl_page(|_| Ok(String::from(data)), "/whatever").expect("Error crawling page");
        assert_eq!(
            result,
            vec!["foo", "/foo", "http://some.domain.com/oeps/bla.html"]
        );
    }
}
