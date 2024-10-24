# Configuration

Ragit is highly configurable. The config files can be found at `.rag_index/configs`, but I don't recommend you modifying it manually. You cannot run any ragit command if there's an error with config files. I have plans to fix this, but for now, just don't do that.

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
// chunks_per_json: 64,
// compression_threshold: 65536,
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
    chunks_per_json: usize,

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
struct QueryConfig {
    // if there are more than this amount of chunks, it runs tf-idf to select chunks
    max_titles: usize,

    // if there are more than this amount of chunks, it runs `rerank_title` prompt to select chunks
    max_summaries: usize,

    // if there are more than this amount of chunks, it runs `rerank_summary` prompt to select chunks
    max_retrieval: usize,
}

// default values
// api_key: None,
// dump_log: false,
// dump_api_usage: true,
// max_retry: 3,
// sleep_between_retries: 20000,
// timeout: 90000,
// sleep_after_llm_call: None,
// model: "llama3.1-70b-groq",
struct ApiConfig {
    api_key: Option<String>,

    // run `rag ls --models` to see the list
    model: String,
    timeout: Option<u64>,
    sleep_between_retries: u64,
    max_retry: usize,
    sleep_after_llm_call: Option<u64>,

    // it records every LLM conversation, including failed ones
    // it's useful if you wanna know what's going on!
    // but be careful, it would take a lot of space
    dump_log: bool,

    // it records how many tokens are used
    dump_api_usage: bool,
}
```
