# Configuration

Ragit is highly configurable. The config files can be found at `.ragit/configs`, but I don't recommend you modifying it manually. If you have modified it manually and have trouble accessing a knowledge-base, try `rag check --recover`.

## Global Configuration

You can set global configuration defaults by placing configuration files in `~/.config/ragit/`. When initializing a new ragit repository, it will check for the following files:

- `~/.config/ragit/api.json` - For API configuration
- `~/.config/ragit/build.json` - For build configuration
- `~/.config/ragit/query.json` - For query configuration

These files can contain a subset of the configuration fields that you want to override. You don't need to include all fields - any fields not specified will use the default values. For example, if you only want to override the `model` and `dump_log` fields in api.json, your file might look like:

```json
{
  "model": "gpt-4o",
  "dump_log": true
}
```

Any values found in these files will override the default values when creating a new repository. This allows you to have consistent configuration across all your ragit repositories.

## `config` command

A recommended way of reading/writing config is `rag config` command.

`rag config --get <KEY>` shows you a value. For example, `rag config --get model` tells you which model you're using.

`rag config --get-all` shows you all the configs.

`rag config --set <KEY> <VALUE>` allows you to set a value.

## Reference

(Dear contributors, below section is auto-generated. Do not modify this manually)

```rust

// default values
// chunk_size: 4000,
// slide_len: 1000,
// image_size: 2000,
// min_summary_len: 200,
// max_summary_len: 1000,
// strict_file_reader: false,
// compression_threshold: 2048,
// compression_level: 3,
struct BuildConfig {
    // it's not a max_chunk_size, and it's impossible to make every chunk have the same size because
    // 1. an image cannot be splitted
    // 2. different files cannot be merged
    // but it's guaranteed that a chunk is never bigger than chunk_size * 2
    chunk_size: usize,

    slide_len: usize,

    // an image is treated like an N characters string
    // this is N
    image_size: usize,

    min_summary_len: usize,
    max_summary_len: usize,

    // If it's set, `rag build` panics if there's any error with a file.
    // For example, if there's an invalid utf-8 character `PlainTextReader` would die.
    // If it cannot follow a link of an image in a markdown file, it would die.
    // You don't need this option unless you're debugging ragit itself.
    strict_file_reader: bool,

    // if the `.chunks` file is bigger than this (in bytes),
    // the file is compressed
    compression_threshold: u64,

    // 0 ~ 9
    compression_level: u32,
}

// default values
// max_titles: 32,
// max_summaries: 10,
// max_retrieval: 3,
// enable_ii: true,
struct QueryConfig {
    /// If there are more than this amount of chunks, it runs tf-idf to select chunks.
    max_titles: usize,

    /// If there are more than this amount of chunks, it runs `rerank_title` prompt to select chunks.
    max_summaries: usize,

    /// If there are more than this amount of chunks, it runs `rerank_summary` prompt to select chunks.
    max_retrieval: usize,

    /// If it's enabled, it uses an inverted index when running tf-idf search.
    /// It doesn't automatically build an inverted index when it's missing. You
    /// have to run `rag ii build` manually to build the index.
    enable_ii: bool,
}

// default values
// api_key: None,
// dump_log: false,
// dump_api_usage: true,
// max_retry: 5,
// sleep_between_retries: 15000,
// timeout: 120000,
// sleep_after_llm_call: None,
// model: "llama3.3-70b-groq",
struct ApiConfig {
    // I recommend you use env var, instead of this.
    api_key: Option<String>,

    // run `rag ls --models` to see the list
    model: String,
    timeout: Option<u64>,
    sleep_between_retries: u64,
    max_retry: usize,

    // in milliseconds
    // if you see 429 too often, use this option
    sleep_after_llm_call: Option<u64>,

    // it records every LLM conversation, including failed ones
    // it's useful if you wanna know what's going on!
    // but be careful, it would take a lot of space
    dump_log: bool,

    // it records how many tokens are used
    dump_api_usage: bool,
}
```
