use crate::uid::Uid;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ChunkSource {
    /// Built directly from a file.
    /// It's `index`th chunk of `path`.
    /// `path` is a relative path.
    File { path: String, index: usize },

    /// TODO: There's an error with this variant: `serde_json` cannot deserialize this.
    ///       The crate can handle u128, but it seems like it cannot handle u128 inside
    ///       a tagged enum. Several issues on this are open on github, and there's even
    ///       a [fix](https://github.com/serde-rs/serde/pull/2781), but it's not merged yet.
    /// Summary of multiple chunks.
    Chunks { uids: Vec<Uid> },
}

impl ChunkSource {
    // this value is directly used to hash this instance
    pub fn hash_str(&self) -> String {
        match self {
            ChunkSource::File { path, index } => format!("{path}{index}"),
            ChunkSource::Chunks { uids } => {
                let mut result = Uid::dummy();

                for chunk_uid in uids.iter() {
                    result ^= *chunk_uid;
                }

                result.to_string()
            },
        }
    }

    pub fn set_path(&mut self, new_path: String) {
        match self {
            ChunkSource::File { path, .. } => { *path = new_path; },
            _ => panic!(),
        }
    }

    pub fn unwrap_index(&self) -> usize {
        match self {
            ChunkSource::File { index, .. } => *index,
            _ => unreachable!(),
        }
    }

    pub fn sortable_string(&self) -> String {
        match self {
            ChunkSource::File { path, index } => format!("file: {path}-{index:09}"),
            ChunkSource::Chunks { .. } => format!("chunks: {}", self.hash_str()),
        }
    }

    pub fn render(&self) -> String {
        match self {
            ChunkSource::File { path, index } => format!(
                "{} chunk of {path}",
                // it's 0-base
                match index {
                    0 => String::from("1st"),
                    1 => String::from("2nd"),
                    2 => String::from("3rd"),
                    n => format!("{}th", n + 1),
                },
            ),
            ChunkSource::Chunks { uids } => format!(
                "multiple chunks ({})",
                uids.iter().map(|uid| uid.to_string().get(0..8).unwrap_or("").to_string()).collect::<Vec<String>>().join(", "),
            ),
        }
    }
}
