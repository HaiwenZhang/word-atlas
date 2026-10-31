//! The canonical vocabulary and its stable ordering, topics and media flags.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// A part of speech with Chinese and English definitions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sense {
    /// Part of speech.
    pub pos: String,
    /// Chinese meaning.
    pub zh: String,
    /// English definition.
    pub en: String,
}

/// A bilingual example supplied by the existing dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    /// English sentence.
    pub en: String,
    /// Chinese translation.
    pub zh: String,
}

/// A complete, immutable dictionary entry; names preserve the web dataset schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Stable identifier used by progress and media files.
    pub id: String,
    /// Headword.
    pub w: String,
    /// British IPA.
    pub uk: String,
    /// American IPA.
    pub us: String,
    /// Definitions, in display order.
    pub senses: Vec<Sense>,
    /// Bilingual examples.
    pub ex: Vec<Example>,
    /// English–Chinese collocations.
    #[serde(default)]
    pub col: Vec<String>,
    /// Related word forms.
    #[serde(default)]
    pub fam: Vec<String>,
    /// Synonyms.
    #[serde(default)]
    pub syn: Vec<String>,
    /// Antonyms.
    #[serde(default)]
    pub ant: Vec<String>,
    /// Usage hint.
    #[serde(default)]
    pub tip: String,
    /// CEFR name, A1 through C2.
    pub cefr: String,
    /// Topic identifier.
    pub topic: String,
    /// Register or usage warning, when present.
    #[serde(default)]
    pub note: String,
    /// Media bit flags: image, UK, US, example, second example.
    #[serde(default)]
    pub m: u8,
}

/// A malformed or inconsistent bundled dictionary.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// JSON cannot be decoded.
    #[error("词库文件无法读取：{0}")]
    Json(#[from] serde_json::Error),
    /// A canonical invariant failed.
    #[error("词库校验失败：{0}")]
    Invalid(String),
}

#[derive(Deserialize)]
struct Index {
    id: String,
    count: usize,
    topics: Vec<(String, String)>,
    cefr: Vec<String>,
    words: Vec<(String, String, String, String, usize, usize, u8)>,
}

/// The complete catalog, indexed by stable ID and kept in the web learning-path order.
#[derive(Debug)]
pub struct Catalog {
    id: String,
    entries: Vec<Entry>,
    positions: BTreeMap<String, usize>,
    topics: Vec<(String, String)>,
    levels: Vec<String>,
}

impl Catalog {
    /// Loads the full repository dataset embedded at build time; no network is used.
    pub fn bundled() -> Result<Self, CatalogError> {
        Self::from_json(
            include_str!("../../../../data/word-atlas.json"),
            include_str!("../../../../public/atlas/index.json"),
        )
    }

    /// Validates canonical entries against the ordering and media index.
    pub fn from_json(entries: &str, index: &str) -> Result<Self, CatalogError> {
        let entries: Vec<Entry> = serde_json::from_str(entries)?;
        let index: Index = serde_json::from_str(index)?;
        let count = entries.len();
        let mut by_id: BTreeMap<_, _> = entries.into_iter().map(|e| (e.id.clone(), e)).collect();
        if count != by_id.len() || count != index.count || count != index.words.len() {
            return Err(CatalogError::Invalid("数量不匹配或有重复词条 ID".into()));
        }
        let mut ordered = Vec::with_capacity(count);
        let mut ids = BTreeSet::new();
        for (id, word, _, _, level, topic, media) in index.words {
            let Some(mut entry) = by_id.remove(&id) else {
                return Err(CatalogError::Invalid(format!("索引缺失或重复：{id}")));
            };
            if !ids.insert(id.clone())
                || entry.w != word
                || entry.senses.is_empty()
                || index.cefr.get(level) != Some(&entry.cefr)
                || index.topics.get(topic).map(|t| &t.0) != Some(&entry.topic)
            {
                return Err(CatalogError::Invalid(format!("条目与索引不一致：{id}")));
            }
            entry.m = media;
            ordered.push(entry);
        }
        let positions = ordered
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.clone(), i))
            .collect();
        Ok(Self {
            id: index.id,
            entries: ordered,
            positions,
            topics: index.topics,
            levels: index.cefr,
        })
    }

    /// The dataset identifier recorded in every progress snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// All entries in the canonical learning order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// An entry by stable identifier.
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.position(id).and_then(|i| self.entries.get(i))
    }
    /// Position on the learning path.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.positions.get(id).copied()
    }
    /// Topic identifiers and display names in the canonical order.
    pub fn topics(&self) -> &[(String, String)] {
        &self.topics
    }
    /// CEFR display names in ascending order.
    pub fn levels(&self) -> &[String] {
        &self.levels
    }
    /// The numeric CEFR level of a word.
    pub fn level(&self, entry: &Entry) -> usize {
        self.levels
            .iter()
            .position(|l| l == &entry.cefr)
            .unwrap_or(0)
    }

    /// Finds words and Chinese meanings, with optional topic and CEFR filters.
    pub fn search<'a>(
        &'a self,
        query: &str,
        topic: Option<&str>,
        level: Option<&str>,
    ) -> Vec<&'a Entry> {
        let query = crate::typing::normalize(query);
        self.entries
            .iter()
            .filter(|entry| {
                topic.is_none_or(|t| entry.topic == t)
                    && level.is_none_or(|l| entry.cefr == l)
                    && (query.is_empty()
                        || crate::typing::normalize(&entry.w).contains(&query)
                        || entry.senses.iter().any(|s| s.zh.contains(&query)))
            })
            .collect()
    }
}
