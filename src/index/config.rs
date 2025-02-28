use serde::{Deserialize, Serialize};

// This struct is used for loading partial configurations from ~/.config/ragit/build.json
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PartialBuildConfig {
    pub chunk_size: Option<usize>,
    pub slide_len: Option<usize>,
    pub image_size: Option<usize>,
    pub min_summary_len: Option<usize>,
    pub max_summary_len: Option<usize>,
    pub strict_file_reader: Option<bool>,
    pub compression_threshold: Option<u64>,
    pub compression_level: Option<u32>,
}

impl PartialBuildConfig {
    // Apply partial config to a full config
    pub fn apply_to(&self, config: &mut BuildConfig) {
        if let Some(chunk_size) = self.chunk_size {
            config.chunk_size = chunk_size;
        }
        if let Some(slide_len) = self.slide_len {
            config.slide_len = slide_len;
        }
        if let Some(image_size) = self.image_size {
            config.image_size = image_size;
        }
        if let Some(min_summary_len) = self.min_summary_len {
            config.min_summary_len = min_summary_len;
        }
        if let Some(max_summary_len) = self.max_summary_len {
            config.max_summary_len = max_summary_len;
        }
        if let Some(strict_file_reader) = self.strict_file_reader {
            config.strict_file_reader = strict_file_reader;
        }
        if let Some(compression_threshold) = self.compression_threshold {
            config.compression_threshold = compression_threshold;
        }
        if let Some(compression_level) = self.compression_level {
            config.compression_level = compression_level;
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct BuildConfig {
    // it's not a max_chunk_size, and it's impossible to make every chunk have the same size because
    // 1. an image cannot be splitted
    // 2. different files cannot be merged
    // but it's guaranteed that a chunk is never bigger than chunk_size * 2
    pub chunk_size: usize,

    pub slide_len: usize,

    // an image is treated like an N characters string
    // this is N
    pub image_size: usize,

    pub min_summary_len: usize,
    pub max_summary_len: usize,

    // If it's set, `rag build` panics if there's any error with a file.
    // For example, if there's an invalid utf-8 character `PlainTextReader` would die.
    // If it cannot follow a link of an image in a markdown file, it would die.
    // You don't need this option unless you're debugging ragit itself.
    pub strict_file_reader: bool,

    // if the `.chunks` file is bigger than this (in bytes),
    // the file is compressed
    pub compression_threshold: u64,

    // 0 ~ 9
    pub compression_level: u32,
}

impl Default for BuildConfig {
    fn default() -> Self {
        BuildConfig {
            chunk_size: 4_000,
            slide_len: 1_000,
            image_size: 2_000,
            min_summary_len: 200,
            max_summary_len: 1000,
            strict_file_reader: false,
            compression_threshold: 2048,
            compression_level: 3,
        }
    }
}
