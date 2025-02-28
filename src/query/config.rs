use serde::{Deserialize, Serialize};

// This struct is used for loading partial configurations from ~/.config/ragit/query.json
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PartialQueryConfig {
    pub max_titles: Option<usize>,
    pub max_summaries: Option<usize>,
    pub max_retrieval: Option<usize>,
    pub enable_ii: Option<bool>,
}

impl PartialQueryConfig {
    // Apply partial config to a full config
    pub fn apply_to(&self, config: &mut QueryConfig) {
        if let Some(max_titles) = self.max_titles {
            config.max_titles = max_titles;
        }
        if let Some(max_summaries) = self.max_summaries {
            config.max_summaries = max_summaries;
        }
        if let Some(max_retrieval) = self.max_retrieval {
            config.max_retrieval = max_retrieval;
        }
        if let Some(enable_ii) = self.enable_ii {
            config.enable_ii = enable_ii;
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct QueryConfig {
    /// If there are more than this amount of chunks, it runs tf-idf to select chunks.
    pub max_titles: usize,

    /// If there are more than this amount of chunks, it runs `rerank_title` prompt to select chunks.
    pub max_summaries: usize,

    /// If there are more than this amount of chunks, it runs `rerank_summary` prompt to select chunks.
    pub max_retrieval: usize,

    /// If it's enabled, it uses an inverted index when running tf-idf search.
    /// It doesn't automatically build an inverted index when it's missing. You
    /// have to run `rag ii build` manually to build the index.
    pub enable_ii: bool,
}

impl Default for QueryConfig {
    fn default() -> Self {
        QueryConfig {
            max_titles: 32,
            max_summaries: 10,
            max_retrieval: 3,
            enable_ii: true,
        }
    }
}
