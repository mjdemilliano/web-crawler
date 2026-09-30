use regex::regex;
use scraper::{Html, Selector};
use std::{env, fmt::Display, vec::Vec};
use ureq;

#[derive(Debug)]
enum Error {
    InvalidURL(String),
    HTTPError(ureq::Error),
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidURL(l0), Self::InvalidURL(r0)) => l0 == r0,
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
            Error::InvalidURL(url) => write!(f, "Invalid URL: {:?}", url),
        }
    }
}

#[derive(Clone, PartialOrd, Ord, Eq, Debug, PartialEq)]
struct URL {
    // Prefix: http[s]://blabla.blabla
    prefix: String,
    // Path: /blog/article
    path: String,
}

impl URL {
    fn as_str(&self) -> String {
        format!("{}{}", self.prefix, self.path)
    }

    fn parse<S: AsRef<str>>(url: S) -> Result<Self, Error> {
        let Some(result) = regex!(r"(([a-z]+)://[^/]+)?(/?.*)").captures(url.as_ref()) else {
            return Err(Error::InvalidURL(url.as_ref().into()));
        };
        let prefix: String = result.get(1).map_or("".into(), |m| m.as_str().into());
        let scheme: String = result.get(2).map_or("".into(), |m| m.as_str().into());
        if !prefix.is_empty() && !(scheme == "http" || scheme == "https") {
            return Err(Error::InvalidURL(url.as_ref().into()));
        }
        let path: String = result.get(3).map_or("".into(), |m| m.as_str().into());
        Ok(URL { prefix, path })
    }
}

impl Display for URL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.prefix, self.path)
    }
}
//
// impl TryFrom<&String> for URL {
//     type Error = Error;
//     fn try_from(url: &String) -> Result<Self, Self::Error> {
//         URL::parse(url)
//     }
// }
//
// impl TryFrom<&str> for URL {
//     type Error = Error;
//     fn try_from(url: &str) -> Result<Self, Self::Error> {
//         URL::parse(url)
//     }
// }

fn crawl_page<G>(page_getter: &G, page_url: &URL) -> Result<Vec<URL>, Error>
where
    G: Fn(&URL) -> Result<String, Error>,
{
    let contents = page_getter(page_url)?;
    let fragment = Html::parse_fragment(&contents);
    let a_selector = Selector::parse("a").unwrap();
    let mut urls: Vec<URL> = fragment
        .select(&a_selector)
        .filter_map(|element| element.value().attr("href"))
        .filter_map(|href| match URL::parse(href) {
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

fn crawl_page_recursive<G>(
    page_getter: &G,
    page_url: &URL,
    already_crawled: &mut Vec<URL>,
) -> Result<Vec<URL>, Error>
where
    G: Fn(&URL) -> Result<String, Error>,
{
    // Add path to be crawled to already crawled.
    already_crawled.push(page_url.clone());
    let mut found_urls: Vec<URL> = crawl_page(page_getter, page_url)?;
    let urls_to_be_crawled: Vec<URL> = found_urls
        .iter()
        .filter(|url| !already_crawled.contains(url))
        // Filter out off-domain links
        .filter(|url| url.prefix == page_url.prefix)
        .map(|url| url.clone())
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

fn http_get(url: &URL) -> Result<String, Error> {
    let mut response = ureq::get(url.as_str())
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
    let Ok(url) = URL::parse(url) else {
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

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_crawl_page() -> Result<(), Error> {
        let data =
            "blabla<a href='foo'>burp</a><a href='/foo'>barf</a><a href='/foo'>barf again</a>
            <p>Blabla</p><a href='http://some.domain.com/oeps/bla.html'>Some link</a>.";
        let result = crawl_page(&|_| Ok(String::from(data)), &URL::parse("/whatever")?)?;
        assert_eq!(
            result,
            vec![
                URL::parse("/foo")?,
                URL::parse("foo")?,
                URL::parse("http://some.domain.com/oeps/bla.html")?
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
                "/" => html_with_links(&[&URL::parse("/foo")?, &URL::parse("/bar")?]),
                "/foo" => html_with_links(&[&URL::parse("/foo/sub")?, &URL::parse("/foo/sub2")?]),
                "/foo/sub" => String::from(""),
                "/foo/sub2" => html_with_links(&[&URL::parse("/bar")?]),
                "/bar" => html_with_links(&[&URL::parse("/foo")?, &URL::parse("/")?]),
                _ => panic!("unexpected url: {}", url),
            };
            Ok(content)
        }

        let mut already_crawled: Vec<URL> = Vec::new();
        let result = crawl_page_recursive(&dummy_http, &URL::parse("/")?, &mut already_crawled)?;
        assert_eq!(
            result,
            vec![
                URL::parse("/")?,
                URL::parse("/bar")?,
                URL::parse("/foo")?,
                URL::parse("/foo/sub")?,
                URL::parse("/foo/sub2")?
            ]
        );
        Ok(())
    }

    #[test]
    fn test_url_parse() {
        assert_eq!(
            URL::parse("http://google.com"),
            Ok(URL {
                prefix: "http://google.com".into(),
                path: "".into()
            })
        );
        assert_eq!(
            URL::parse("http://google.com/"),
            Ok(URL {
                prefix: "http://google.com".into(),
                path: "/".into()
            })
        );
        assert_eq!(
            URL::parse("https://google.com"),
            Ok(URL {
                prefix: "https://google.com".into(),
                path: "".into()
            })
        );
        assert_eq!(
            URL::parse("https://sub.somedomain.com/foo/bar.html"),
            Ok(URL {
                prefix: "https://sub.somedomain.com".into(),
                path: "/foo/bar.html".into()
            })
        );
        assert_eq!(
            URL::parse(""),
            Ok(URL {
                prefix: "".into(),
                path: "".into()
            })
        );
        assert_eq!(
            URL::parse("/"),
            Ok(URL {
                prefix: "".into(),
                path: "/".into()
            })
        );
        assert!(URL::parse("burp://google.com").is_err());
        assert!(URL::parse("burp://google.com/").is_err());
        assert_eq!(
            URL::parse("http://somedomain.nl/foo"),
            Ok(URL {
                prefix: "http://somedomain.nl".into(),
                path: "/foo".into()
            })
        );
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
                    &URL::parse("/foo")?,
                    &URL::parse("http://someother.domain.com/bar")?,
                ]),
                "/foo" => html_with_links(&[&URL::parse("/foo/sub")?, &URL::parse("/foo/sub2")?]),
                "/foo/sub" => String::from(""),
                "/foo/sub2" => String::from(""),
                _ => panic!("unexpected url: {}", url),
            };
            Ok(content)
        }

        let mut already_crawled: Vec<URL> = Vec::new();
        let result = crawl_page_recursive(&dummy_http, &URL::parse("/")?, &mut already_crawled)?;
        assert_eq!(
            result,
            vec![
                URL::parse("/foo")?,
                URL::parse("/foo/sub")?,
                URL::parse("/foo/sub2")?,
                URL::parse("http://someother.domain.com/bar")?,
            ]
        );
        Ok(())
    }
}
