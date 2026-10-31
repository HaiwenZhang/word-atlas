//! Inflection-aware sentence clozes, preserving the existing web application's matching rules.

use std::{collections::BTreeMap, sync::OnceLock};

use regex::Regex;

/// The three original sentence fragments around a headword or its inflected form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cloze<'a> {
    /// Text before the matching word.
    pub before: &'a str,
    /// The original matched word, including its inflection and capitalization.
    pub hidden: &'a str,
    /// Text following the word.
    pub after: &'a str,
}

fn irregular() -> &'static BTreeMap<&'static str, Vec<&'static str>> {
    static FORMS: OnceLock<BTreeMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    FORMS.get_or_init(|| {
        include_str!("irregular-forms.txt")
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let base = words.next()?;
                Some((base, words.collect()))
            })
            .collect()
    })
}

/// Finds a headword without matching inside a longer, unrelated ASCII word.
pub fn gap<'a>(sentence: &'a str, word: &str) -> Option<Cloze<'a>> {
    let word = word.trim().to_lowercase();
    if word.is_empty() {
        return None;
    }
    let pattern = if word.chars().any(char::is_whitespace) {
        word.split_whitespace()
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join(r"\s+")
    } else if let Some(forms) = irregular().get(word.as_str()) {
        let options = std::iter::once(word.as_str())
            .chain(forms.iter().copied())
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join("|");
        format!("(?:{options})[a-z]{{0,3}}")
    } else {
        let letters: Vec<_> = word.chars().collect();
        let n = letters.len();
        let vowel = |ch: char| "aeiou".contains(ch);
        let stem = if n > 3
            && (letters[n - 1] == 'e' || (letters[n - 1] == 'y' && !vowel(letters[n - 2])))
        {
            letters[..n - 1].iter().collect::<String>()
        } else {
            word.clone()
        };
        let doubled = if n >= 3
            && !vowel(letters[n - 3])
            && vowel(letters[n - 2])
            && "bdgmnprt".contains(letters[n - 1])
        {
            format!("{}?", regex::escape(&letters[n - 1].to_string()))
        } else {
            String::new()
        };
        format!("{}{doubled}[a-z]{{0,4}}", regex::escape(&stem))
    };
    // The trailing boundary lets alternations retry the longer stem (spin/spinn -> spinning).
    let matcher = Regex::new(&format!("(?i:({pattern}))($|[^A-Za-z])")).ok()?;
    for captures in matcher.captures_iter(sentence) {
        let matched = captures.get(1)?;
        let (start, end) = (matched.start(), matched.end());
        let before = &sentence[..start];
        let after = &sentence[end..];
        if before
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
            || after
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            continue;
        }
        return Some(Cloze {
            before,
            hidden: &sentence[start..end],
            after,
        });
    }
    None
}
