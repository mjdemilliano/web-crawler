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

fn crawl_page<G>(page_getter: &G, url: URLRef) -> Result<Vec<URL>, Error>
where
    G: Fn(URLRef) -> Result<String, Error>,
{
    let contents = page_getter(url)?;
    let fragment = Html::parse_fragment(&contents);
    let a_selector = Selector::parse("a").unwrap();
    let mut urls: Vec<URL> = fragment
        .select(&a_selector)
        .filter_map(|element| element.value().attr("href"))
        .map(|href| String::from(href))
        .collect();
    urls.sort();
    urls.dedup();
    Ok(urls)
}

fn crawl_page_recursive<G>(
    page_getter: &G,
    url: URLRef,
    already_crawled: &mut Vec<URL>,
) -> Result<Vec<URL>, Error>
where
    G: Fn(URLRef) -> Result<String, Error>,
{
    // Add URL to be crawled to already crawled.
    already_crawled.push(URL::from(url));
    let mut found_urls: Vec<URL> = crawl_page(page_getter, url)?;
    let urls_to_be_crawled: Vec<URL> = found_urls
        .iter()
        .filter(|url| !already_crawled.contains(url))
        .map(|url| URL::from(url))
        .collect();
    // Now recursively crawl all URLs not yet crawled.
    for url in urls_to_be_crawled {
        let mut new_urls = crawl_page_recursive(page_getter, &url, already_crawled)?;
        new_urls.sort();
        new_urls.dedup();
        found_urls.extend(new_urls);
    }
    found_urls.sort();
    found_urls.dedup();
    Ok(found_urls)
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
    let mut already_crawled = Vec::<URL>::new();
    let urls = match crawl_page_recursive(&http_get, &url, &mut already_crawled) {
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
        let data =
            "blabla<a href='foo'>burp</a><a href='/foo'>barf</a><a href='/foo'>barf again</a>
            <p>Blabla</p><a href='http://some.domain.com/oeps/bla.html'>Some link</a>.";
        let result =
            crawl_page(&|_| Ok(String::from(data)), "/whatever").expect("Error crawling page");
        assert_eq!(
            result,
            vec!["/foo", "foo", "http://some.domain.com/oeps/bla.html"]
        );
    }

    #[test]
    fn test_crawl_page_recursive() {
        // Test structure:
        //   - / mentions /foo and /bar
        //   - /foo mentions /foo/sub and /foo/sub2
        //   - /foo/sub has no URLs
        //   - /foo/sub2 mentions /bar
        //   - /bar mentions /foo and /
        fn dummy_http(url: URLRef) -> Result<String, Error> {
            fn html_with_links(urls: &[URLRef]) -> String {
                let a_hrefs = urls
                    .iter()
                    .map(|url| String::from(format!("<a href='{}'>Some link</a>", url)));
                a_hrefs.collect()
            }
            let content = match url {
                "/" => html_with_links(&["/foo", "/bar"]),
                "/foo" => html_with_links(&["/foo/sub", "/foo/sub2"]),
                "/foo/sub" => String::from(""),
                "/foo/sub2" => html_with_links(&["/bar"]),
                "/bar" => html_with_links(&["/foo", "/"]),
                _ => panic!("unexpected url: {}", url),
            };
            Ok(content)
        }

        let mut already_crawled: Vec<URL> = Vec::new();
        let result = crawl_page_recursive(&dummy_http, "/", &mut already_crawled)
            .expect("Error crawling page");
        assert_eq!(result, vec!["/", "/bar", "/foo", "/foo/sub", "/foo/sub2"]);
    }
}
