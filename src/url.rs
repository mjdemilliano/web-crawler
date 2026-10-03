use regex::regex;
use std::fmt::Display;

#[derive(Clone, PartialOrd, Ord, Eq, Debug, PartialEq)]
pub struct URL {
    // Prefix: http[s]://blabla.blabla
    pub prefix: String,
    // Path: /blog/article
    pub path: String,
}

#[derive(PartialEq, Debug)]
pub enum Error {
    InvalidURL(String),
}

impl URL {
    pub fn as_str(&self) -> String {
        format!("{}{}", self.prefix, self.path)
    }

    pub fn parse<S: AsRef<str> + std::fmt::Display>(url: S) -> Self {
        URL::try_parse(url.as_ref()).expect(&format!("parsing URL failed: {}", url.as_ref()))
    }

    pub fn try_parse<S: AsRef<str>>(url: S) -> Result<Self, Error> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_parse() {
        assert_eq!(
            URL::parse("http://google.com"),
            URL {
                prefix: "http://google.com".into(),
                path: "".into()
            }
        );
        assert_eq!(
            URL::parse("http://google.com/"),
            URL {
                prefix: "http://google.com".into(),
                path: "/".into()
            }
        );
        assert_eq!(
            URL::parse("https://google.com"),
            URL {
                prefix: "https://google.com".into(),
                path: "".into()
            }
        );
        assert_eq!(
            URL::parse("https://sub.somedomain.com/foo/bar.html"),
            URL {
                prefix: "https://sub.somedomain.com".into(),
                path: "/foo/bar.html".into()
            }
        );
        assert_eq!(
            URL::parse(""),
            URL {
                prefix: "".into(),
                path: "".into()
            }
        );
        assert_eq!(
            URL::parse("/"),
            URL {
                prefix: "".into(),
                path: "/".into()
            }
        );
        assert!(URL::try_parse("burp://google.com").is_err());
        assert!(URL::try_parse("burp://google.com/").is_err());
        assert_eq!(
            URL::parse("http://somedomain.nl/foo"),
            URL {
                prefix: "http://somedomain.nl".into(),
                path: "/foo".into()
            }
        );
    }
}
