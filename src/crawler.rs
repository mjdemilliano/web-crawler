use scraper::{Html, Selector};
use std::{collections::HashSet, fmt::Display, vec::Vec};
use ureq;

use crate::url::{self, URL};

#[derive(Debug)]
pub enum Error {
    URLError(url::Error),
    HTTPError(ureq::Error),
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::URLError(l0), Self::URLError(r0)) => l0 == r0,
            // PartialEq not implemented for HTTPError, but we can do string comparison to get the same effect
            (Self::HTTPError(l0), Self::HTTPError(r0)) => {
                format!("{:?}", l0) == format!("{:?}", r0)
            }
            _ => false,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::HTTPError(error) => write!(f, "HTTP error: {:?}", error),
            Error::URLError(url::Error::InvalidURL(url)) => write!(f, "Invalid URL: {:?}", url),
        }
    }
}

pub fn crawl_page<G>(page_getter: &G, page_url: &URL) -> Result<Vec<URL>, Error>
where
    G: Fn(&URL) -> Result<String, Error>,
{
    let contents = page_getter(page_url)?;
    let fragment = Html::parse_fragment(&contents);
    let a_selector = Selector::parse("a").unwrap();
    let mut urls: Vec<URL> = fragment
        .select(&a_selector)
        .filter_map(|element| element.value().attr("href"))
        .filter_map(|href| match URL::try_parse(href) {
            Ok(url) => Some(url),
            Err(_) => None, // Ignore garbage URLs
        })
        .map(|url| {
            // If only a path is provided, use prefix of the crawled page.
            if url.prefix.is_empty() {
                return URL {
                    prefix: page_url.prefix.clone(),
                    path: url.path,
                };
            } else {
                return url;
            }
        })
        .collect();
    urls.sort();
    urls.dedup();
    Ok(urls)
}

fn display_urls<I>(urls: I) -> String
where
    I: IntoIterator,
    I::Item: AsRef<URL>,
{
    let mut result = String::new();
    let mut items = urls.into_iter();
    if let Some(url) = items.next() {
        result.push_str(&url.as_ref().as_str());
    }
    while let Some(url) = items.next() {
        result.push_str(", ");
        result.push_str(&url.as_ref().as_str());
    }
    result
}

/// Crawls a page recursively.
/// If crawl_budget is provided, then at most that number of pages is crawled.
/// The crawl budget does not limit the number of URLs returned.
pub fn crawl_page_recursive<G>(
    page_getter: &G,
    page_url: &URL,
    crawl_budget: Option<usize>,
) -> Result<Vec<URL>, Error>
where
    G: Fn(&URL) -> Result<String, Error>,
{
    let mut all_new_found_urls: HashSet<URL> = HashSet::new();
    let mut already_crawled: HashSet<URL> = HashSet::new();
    let mut urls_to_crawl: HashSet<URL> = HashSet::new();
    urls_to_crawl.insert(page_url.clone());
    let mut my_crawl_budget = crawl_budget.clone();
    while my_crawl_budget.is_none_or(|budget| budget > 0) && !urls_to_crawl.is_empty() {
        // println!("\n<<iteration>>");
        let url = urls_to_crawl.iter().next().unwrap().clone();
        let urls_on_page: HashSet<URL> =
            HashSet::from_iter(crawl_page(page_getter, &url)?.into_iter());
        // println!(
        //     "crawled page {}, got: {}",
        //     url,
        //     display_urls(urls_on_page.iter())
        // );
        if my_crawl_budget.is_some() {
            my_crawl_budget = Some(my_crawl_budget.unwrap() - 1);
        }
        already_crawled.insert(url.clone());
        urls_to_crawl.remove(&url);
        // println!("already crawled = {}", display_urls(already_crawled.iter()));
        let new_urls: Vec<URL> = urls_on_page
            .difference(&already_crawled)
            .map(|x| x.clone())
            .collect();
        // println!("new urls = {}", display_urls(new_urls.iter()));
        urls_to_crawl.extend(
            new_urls
                .iter()
                // Do not crawl off-domain
                .filter(|url| url.prefix == page_url.prefix)
                .map(|url| url.clone()),
        );
        // println!("urls to crawl = {}", display_urls(urls_to_crawl.iter()));
        all_new_found_urls.extend(new_urls.iter().map(|url| url.clone()));
        // println!(
        //     "all new found urls = {}",
        //     display_urls(all_new_found_urls.iter())
        // );
    }
    let mut result = Vec::from_iter(all_new_found_urls.iter().cloned());
    result.sort();
    Ok(result)
}

pub fn http_get(url: &URL) -> Result<String, Error> {
    let mut response = ureq::get(url.as_str())
        .call()
        .map_err(|error| Error::HTTPError(error))?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|error| Error::HTTPError(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crawl_page() -> Result<(), url::Error> {
        let data =
            "blabla<a href='foo'>burp</a><a href='/foo'>barf</a><a href='/foo'>barf again</a>
            <p>Blabla</p><a href='http://some.domain.com/oeps/bla.html'>Some link</a>.";
        let result = crawl_page(&|_| Ok(String::from(data)), &URL::parse("/whatever"))
            .expect("crawl page failed");
        assert_eq!(
            result,
            vec![
                URL::parse("/foo"),
                URL::parse("foo"),
                URL::parse("http://some.domain.com/oeps/bla.html")
            ]
        );
        Ok(())
    }

    #[test]
    fn test_crawl_page_recursive() -> Result<(), Error> {
        // Test structure:
        //   - / mentions /foo and /bar
        //   - /foo mentions /foo/sub and /foo/sub2
        //   - /foo/sub has no URLs
        //   - /foo/sub2 mentions /bar
        //   - /bar mentions /foo and /
        fn dummy_http(url: &URL) -> Result<String, Error> {
            fn html_with_links(urls: &[&URL]) -> String {
                let a_hrefs = urls
                    .iter()
                    .map(|url| String::from(format!("<a href='{}'>Some link</a>", url)));
                a_hrefs.collect()
            }
            let content = match url.path.as_ref() {
                "/" => html_with_links(&[&URL::parse("/foo"), &URL::parse("/bar")]),
                "/foo" => html_with_links(&[&URL::parse("/foo/sub"), &URL::parse("/foo/sub2")]),
                "/foo/sub" => String::from(""),
                "/foo/sub2" => html_with_links(&[&URL::parse("/bar")]),
                "/bar" => html_with_links(&[&URL::parse("/foo"), &URL::parse("/")]),
                _ => panic!("unexpected url: {}", url),
            };
            Ok(content)
        }

        let result =
            crawl_page_recursive(&dummy_http, &URL::parse("/"), None).expect("crawl page failed");
        assert_eq!(
            result,
            vec![
                URL::parse("/bar"),
                URL::parse("/foo"),
                URL::parse("/foo/sub"),
                URL::parse("/foo/sub2")
            ]
        );
        Ok(())
    }

    #[test]
    fn test_crawl_recursive_does_not_crawl_off_domain() -> Result<(), Error> {
        fn dummy_http(url: &URL) -> Result<String, Error> {
            fn html_with_links(urls: &[&URL]) -> String {
                let a_hrefs = urls
                    .iter()
                    .map(|url| String::from(format!("<a href='{}'>Some link</a>", url)));
                a_hrefs.collect()
            }
            if !url.prefix.is_empty() {
                return Err(Error::HTTPError(ureq::Error::HostNotFound));
            }
            let content = match url.path.as_ref() {
                "/" => html_with_links(&[
                    &URL::parse("/foo"),
                    &URL::parse("http://someother.domain.com/bar"),
                ]),
                "/foo" => html_with_links(&[&URL::parse("/foo/sub"), &URL::parse("/foo/sub2")]),
                "/foo/sub" => String::from(""),
                "/foo/sub2" => String::from(""),
                _ => panic!("unexpected url: {}", url),
            };
            Ok(content)
        }

        let result = crawl_page_recursive(&dummy_http, &URL::parse("/"), None)?;
        assert_eq!(
            result,
            vec![
                URL::parse("/foo"),
                URL::parse("/foo/sub"),
                URL::parse("/foo/sub2"),
                URL::parse("http://someother.domain.com/bar"),
            ]
        );
        Ok(())
    }

    #[test]
    fn test_crawl_page_recursive_limited() -> Result<(), Error> {
        // Test structure:
        //   - / mentions /foo and /bar
        //   - /foo mentions /foo/sub and /foo/sub2 and /foo/sub3
        //   - /foo/sub has no URLs
        //   - /foo/sub2 mentions /baz/1..10
        fn dummy_http(url: &URL) -> Result<String, Error> {
            fn html_with_links(urls: &[URL]) -> String {
                let a_hrefs = urls
                    .iter()
                    .map(|url| String::from(format!("<a href='{}'>Some link</a>", url)));
                a_hrefs.collect()
            }
            let content = match url.path.as_ref() {
                "/" => html_with_links(&[URL::parse("/foo"), URL::parse("/bar")]),
                "/foo" => html_with_links(&[URL::parse("/foo/sub"), URL::parse("/foo/sub2")]),
                "/foo/sub" => String::from(""),
                "/foo/sub2" => {
                    let urls: Vec<URL> = (1..=10)
                        .map(|x| URL::parse(format!("/baz/{}", x)))
                        .collect();
                    html_with_links(&urls)
                }
                "/bar" => html_with_links(&[URL::parse("/foo"), URL::parse("/")]),
                x if x.starts_with("/baz/") => String::from(""),
                _ => panic!("unexpected url: {}", url),
            };
            Ok(content)
        }

        assert_eq!(
            crawl_page_recursive(&dummy_http, &URL::parse("/"), Some(0))
                .expect("crawl page failed"),
            vec![]
        );
        assert_eq!(
            crawl_page_recursive(&dummy_http, &URL::parse("/"), Some(1))
                .expect("crawl page failed"),
            vec![URL::parse("/bar"), URL::parse("/foo"),]
        );
        Ok(())
    }
}
