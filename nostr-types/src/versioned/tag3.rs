use crate::types::{ParsedTag, UncheckedUrl};
use crate::Error;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};

/// A tag on an Event
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct TagV3(Vec<String>);

impl TagV3 {
    const EMPTY_STRING: &'static str = "";

    /// Create a new tag
    pub fn new(fields: &[&str]) -> TagV3 {
        TagV3(fields.iter().map(|f| (*f).to_owned()).collect())
    }

    /// Create a new tag without copying
    pub fn from_strings(fields: Vec<String>) -> TagV3 {
        TagV3(fields)
    }

    /// Remove empty fields from the end
    pub fn trim(&mut self) {
        while self.0[self.len() - 1].is_empty() {
            let _ = self.0.pop();
        }
    }

    /// Into a `Vec<String>`
    pub fn into_inner(self) -> Vec<String> {
        self.0
    }

    /// Number of string fields in the tag
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is the tag empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the string at the given index
    pub fn get_index(&self, index: usize) -> &str {
        if self.len() > index {
            &self.0[index]
        } else {
            Self::EMPTY_STRING
        }
    }

    /// Get the string at the given index, None if beyond length or empty
    pub fn get_opt_index(&self, i: usize) -> Option<&str> {
        if self.0.len() <= i {
            None
        } else {
            let s = self.get_index(i);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }

    /// Set the string at the given index
    pub fn set_index(&mut self, index: usize, value: String) {
        while self.len() <= index {
            self.0.push("".to_owned());
        }
        self.0[index] = value;
    }

    /// Push another values onto the tag
    pub fn push_value(&mut self, value: String) {
        self.0.push(value);
    }

    /// Push more values onto the tag
    pub fn push_values(&mut self, mut values: Vec<String>) {
        for value in values.drain(..) {
            self.0.push(value);
        }
    }

    /// Get the tag name for the tag (the first string in the array)
    pub fn tagname(&self) -> &str {
        self.get_index(0)
    }

    /// Get the tag value (index 1, after the tag name)
    pub fn value(&self) -> &str {
        self.get_index(1)
    }

    /// Get the marker (if relevant), else ""
    pub fn marker(&self) -> &str {
        if self.tagname() == "e" || self.tagname() == "a" {
            self.get_index(3)
        } else {
            Self::EMPTY_STRING
        }
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> TagV3 {
        TagV3(vec!["e".to_string(), UncheckedUrl::mock().0])
    }

    /// Parse into a ParsedTag
    pub fn parse(&self) -> Result<ParsedTag, Error> {
        ParsedTag::parse(self)
    }
}
