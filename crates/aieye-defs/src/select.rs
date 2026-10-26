//! Selectors: where in a config file the entries a definition needs are.
//!
//! A selector is a list of keys separated by `.`, walked from the top of the file.
//! `*` stands for each key of an object, or each item of a list. For example,
//! `projects.*.mcpServers.*` selects every MCP server of every project.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

/// One step of a selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// The value under this key.
    Key(String),
    /// Each value of an object or list.
    Any,
}

/// A parsed selector, e.g. `projects.*`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Selector(Vec<Segment>);

impl Selector {
    pub fn segments(&self) -> &[Segment] {
        &self.0
    }

    /// Whether the last step is `*`, so that each match has a key of its own.
    pub fn ends_with_any(&self) -> bool {
        self.0.last() == Some(&Segment::Any)
    }
}

impl FromStr for Selector {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        text.split('.')
            .map(|step| match step {
                "" => Err(format!("selector `{text}` has an empty step")),
                "*" => Ok(Segment::Any),
                key if key.contains('*') => {
                    Err(format!("selector `{text}`: `*` must be a whole step"))
                }
                key => Ok(Segment::Key(key.to_owned())),
            })
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

impl TryFrom<String> for Selector {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse()
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(".")?;
            }
            match segment {
                Segment::Key(key) => f.write_str(key)?,
                Segment::Any => f.write_str("*")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys_and_wildcards() {
        let selector: Selector = "projects.*.mcpServers.*".parse().unwrap();
        assert_eq!(
            selector.segments(),
            [
                Segment::Key("projects".into()),
                Segment::Any,
                Segment::Key("mcpServers".into()),
                Segment::Any,
            ]
        );
        assert!(selector.ends_with_any());
        assert_eq!(selector.to_string(), "projects.*.mcpServers.*");
    }

    #[test]
    fn rejects_empty_steps_and_partial_wildcards() {
        for text in ["", "a..b", ".a", "a.", "a*", "*b"] {
            assert!(text.parse::<Selector>().is_err(), "{text}");
        }
    }
}
