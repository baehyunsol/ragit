use crate::chunk::{Chunk, RenderableChunk, merge_and_convert_chunks};
use crate::error::Error;
use crate::index::Index;
use ragit_api::{
    RecordAt,
    Request,
};
use ragit_pdl::{
    Pdl,
    escape_pdl_tokens,
    parse_pdl,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod config;
mod keyword;

pub use config::QueryConfig;
pub use keyword::Keywords;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QueryResponse {
    pub multi_turn_schema: Option<MultiTurnSchema>,
    pub retrieved_chunks: Vec<Chunk>,
    pub response: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QueryTurn {
    pub query: String,
    pub response: QueryResponse,
}

impl QueryTurn {
    pub fn new(query: String, response: QueryResponse) -> Self {
        QueryTurn { query, response }
    }
}

impl Index {
    pub async fn retrieve_chunks(&self, query: &str) -> Result<Vec<Chunk>, Error> {
        let mut chunks = self.load_chunks_or_tfidf(query).await?;

        if chunks.len() > self.query_config.max_summaries {
            chunks = self.titles_to_summaries(
                query,
                chunks.into_iter().map(|c| c.into()).collect(),
            ).await?;
        }

        if chunks.len() > self.query_config.max_retrieval {
            chunks = self.summaries_to_chunks(
                query,
                chunks.into_iter().map(|c| c.into()).collect(),
                self.query_config.max_retrieval,
            ).await?;
        }

        Ok(chunks)
    }

    /// A simple version of `query`, in case you're asking only a single question.
    pub async fn single_turn(
        &self,
        q: &str,
    ) -> Result<String, Error> {
        let result = self.query(q, vec![]).await?;
        Ok(result.response)
    }

    pub async fn query(
        &self,
        q: &str,
        history: Vec<QueryTurn>,
    ) -> Result<QueryResponse, Error> {
        let (multi_turn_schema, query) = if history.is_empty() {
            (None, q.to_string())
        } else {
            let multi_turn_schema = self.rephrase_multi_turn(
                select_turns_for_context(&history, q),
            ).await?;
            let query = if multi_turn_schema.is_query && multi_turn_schema.in_context {
                multi_turn_schema.query.clone()
            } else {
                q.to_string()
            };

            (Some(multi_turn_schema), query)
        };
        let chunks = self.retrieve_chunks(&query).await?;

        let response = if chunks.is_empty() {
            let mut history_turns = Vec::with_capacity(history.len() * 2);

            for h in history.iter() {
                history_turns.push(h.query.clone());
                history_turns.push(h.response.response.clone());
            }

            self.raw_request(
                q,
                history_turns,
            ).await?
        } else {
            self.answer_query_with_chunks(
                &query,
                merge_and_convert_chunks(self, chunks.clone())?,
            ).await?
        };

        Ok(QueryResponse {
            multi_turn_schema,
            retrieved_chunks: chunks,
            response,
        })
    }

    pub async fn titles_to_summaries(
        &self,
        query: &str,
        chunks: Vec<Chunk>,
    ) -> Result<Vec<Chunk>, Error> {
        let mut tera_context = tera::Context::new();
        tera_context.insert(
            "titles",
            &chunks.iter().enumerate().map(
                |(index, chunk)| format!("{}. {}", index + 1, escape_pdl_tokens(&chunk.title))
            ).collect::<Vec<_>>().join("\n"),
        );
        tera_context.insert(
            "query",
            &escape_pdl_tokens(&query),
        );
        tera_context.insert(
            "max_index",
            &chunks.len(),
        );

        // It's good to allow LLMs choose as many chunks as possible.
        // But allowing it to choose all the chunks might lead to an infinite loop.
        tera_context.insert(
            "max_retrieval",
            &(chunks.len() - 1),
        );

        let Pdl { messages, schema } = parse_pdl(
            &self.get_prompt("rerank_title")?,
            &tera_context,
            "/",  // TODO: `<|media|>` is not supported for this prompt
            true,
            true,
        )?;
        let request = Request {
            messages,
            frequency_penalty: None,
            max_tokens: None,
            temperature: None,
            timeout: self.api_config.timeout,
            max_retry: self.api_config.max_retry,
            sleep_between_retries: self.api_config.sleep_between_retries,
            dump_pdl_at: self.api_config.create_pdl_path("rerank_title"),
            dump_json_at: self.api_config.dump_log_at.clone(),
            model: self.get_model_by_name(&self.api_config.model)?,
            record_api_usage_at: self.api_config.dump_api_usage_at.clone().map(
                |path| RecordAt { path, id: String::from("rerank_title") }
            ),
            schema,
            schema_max_try: 3,
        };
        let title_indices = request.send_and_validate::<Vec<usize>>(vec![]).await?;

        Ok(chunks.into_iter().enumerate().filter(
            |(index, _)| title_indices.contains(&(index + 1))
        ).map(
            |(_, chunk)| chunk
        ).collect())
    }

    pub async fn summaries_to_chunks(
        &self,
        query: &str,
        chunks: Vec<Chunk>,
        max_retrieval: usize,
    ) -> Result<Vec<Chunk>, Error> {
        let mut tera_context = tera::Context::new();
        tera_context.insert(
            "entries",
            &chunks.iter().enumerate().map(
                |(index, chunk)| format!(
                    "{}. {}\nsource: {}\nsummary: {}",
                    index + 1,
                    escape_pdl_tokens(&chunk.title),
                    escape_pdl_tokens(&chunk.render_source()),
                    escape_pdl_tokens(&chunk.summary),
                )
            ).collect::<Vec<_>>().join("\n\n"),
        );
        tera_context.insert(
            "query",
            &escape_pdl_tokens(&query),
        );
        tera_context.insert(
            "max_retrieval",
            &max_retrieval,
        );
        tera_context.insert(
            "max_index",
            &chunks.len(),
        );

        let Pdl { messages, schema } = parse_pdl(
            &self.get_prompt("rerank_summary")?,
            &tera_context,
            "/",  // TODO: `<|media|>` is not supported for this prompt
            true,
            true,
        )?;
        let request = Request {
            messages,
            frequency_penalty: None,
            max_tokens: None,
            temperature: None,
            timeout: self.api_config.timeout,
            max_retry: self.api_config.max_retry,
            sleep_between_retries: self.api_config.sleep_between_retries,
            dump_pdl_at: self.api_config.create_pdl_path("rerank_summary"),
            dump_json_at: self.api_config.dump_log_at.clone(),
            model: self.get_model_by_name(&self.api_config.model)?,
            record_api_usage_at: self.api_config.dump_api_usage_at.clone().map(
                |path| RecordAt { path, id: String::from("rerank_summary") }
            ),
            schema,
            schema_max_try: 3,
        };
        let chunk_indices = request.send_and_validate::<Vec<usize>>(vec![]).await?;

        Ok(chunks.into_iter().enumerate().filter(
            |(index, _)| chunk_indices.contains(&(index + 1))
        ).map(
            |(_, chunk)| chunk
        ).collect())
    }

    pub async fn answer_query_with_chunks(
        &self,
        query: &str,
        chunks: Vec<RenderableChunk>,
    ) -> Result<String, Error> {
        let mut tera_context = tera::Context::new();
        tera_context.insert(
            "chunks",
            &chunks,  // it's already escaped
        );
        tera_context.insert(
            "query",
            &escape_pdl_tokens(&query),
        );

        let Pdl { messages, .. } = parse_pdl(
            &self.get_prompt("answer_query")?,
            &tera_context,
            "/",  // TODO: `<|media|>` is not supported for this prompt
            true,
            true,
        )?;

        let request = Request {
            messages,
            timeout: self.api_config.timeout,
            max_retry: self.api_config.max_retry,
            sleep_between_retries: self.api_config.sleep_between_retries,
            dump_pdl_at: self.api_config.create_pdl_path("answer_query_with_chunks"),
            dump_json_at: self.api_config.dump_log_at.clone(),
            model: self.get_model_by_name(&self.api_config.model)?,
            record_api_usage_at: self.api_config.dump_api_usage_at.clone().map(
                |path| RecordAt { path, id: String::from("answer_query_with_chunks") }
            ),
            schema: None,
            ..Request::default()
        };

        let response = request.send().await?;

        Ok(response.get_message(0).unwrap().to_string())
    }

    pub async fn rephrase_multi_turn(
        &self,
        turns: Vec<String>,
    ) -> Result<MultiTurnSchema, Error> {
        let turns_json = Value::Array(turns.iter().map(|turn| Value::String(escape_pdl_tokens(turn))).collect());
        let turns_json = serde_json::to_string_pretty(&turns_json)?;
        let mut tera_context = tera::Context::new();
        tera_context.insert("turns", &turns_json);

        let Pdl { messages, schema } = parse_pdl(
            &self.get_prompt("multi_turn")?,
            &tera_context,
            "/",  // TODO: `<|media|>` is not supported for this prompt
            true,
            true,
        )?;

        let request = Request {
            messages,
            frequency_penalty: None,
            max_tokens: None,
            temperature: None,
            timeout: self.api_config.timeout,
            max_retry: self.api_config.max_retry,
            sleep_between_retries: self.api_config.sleep_between_retries,
            dump_pdl_at: self.api_config.create_pdl_path("rephrase_multi_turn"),
            dump_json_at: self.api_config.dump_log_at.clone(),
            model: self.get_model_by_name(&self.api_config.model)?,
            record_api_usage_at: self.api_config.dump_api_usage_at.clone().map(
                |path| RecordAt { path, id: String::from("rephrase_multi_turn") }
            ),
            schema,
            schema_max_try: 3,
        };

        Ok(request.send_and_validate::<MultiTurnSchema>(MultiTurnSchema::default()).await?)
    }

    pub async fn raw_request(
        &self,
        query: &str,
        history: Vec<String>,
    ) -> Result<String, Error> {
        let mut tera_context = tera::Context::new();
        tera_context.insert("query", &escape_pdl_tokens(&query));
        tera_context.insert("history", &history.iter().map(|h| escape_pdl_tokens(h)).collect::<Vec<_>>());

        let Pdl { messages, .. } = parse_pdl(
            &self.get_prompt("raw")?,
            &tera_context,
            "/",  // TODO: `<|media|>` is not supported for this prompt
            true,
            true,
        )?;
        let request = Request {
            messages,
            timeout: self.api_config.timeout,
            max_retry: self.api_config.max_retry,
            sleep_between_retries: self.api_config.sleep_between_retries,
            dump_pdl_at: self.api_config.create_pdl_path("raw_request"),
            dump_json_at: self.api_config.dump_log_at.clone(),
            model: self.get_model_by_name(&self.api_config.model)?,
            record_api_usage_at: self.api_config.dump_api_usage_at.clone().map(
                |path| RecordAt { path, id: String::from("raw_request") }
            ),
            schema: None,
            ..Request::default()
        };

        let response = request.send().await?;
        Ok(response.get_message(0).unwrap().to_string())
    }
}

/// Ragit supports multi-turn conversations. Since the pipeline
/// can handle only 1 query at a time, a multi-turn conversation
/// has to be rephrased into a single query. Ragit uses a
/// prompt to do that. This struct is a result of the prompt.
/// If `is_query` is set, the rephrasing is successful and you
/// can find the query in `query`. Otherwise, the last turn of
/// the conversation is not a query.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MultiTurnSchema {
    pub is_query: bool,
    pub in_context: bool,
    pub query: String,
}

impl Default for MultiTurnSchema {
    fn default() -> Self {
        MultiTurnSchema {
            is_query: false,
            in_context: false,
            query: String::new(),
        }
    }
}

fn select_turns_for_context(history: &[QueryTurn], query: &str) -> Vec<String> {
    match history.len() {
        0 => unreachable!(),
        1 => vec![
            history[0].query.to_string(),
            history[0].response.response.to_string(),
            query.to_string(),
        ],
        _ => {
            let last_turn = history.last().unwrap();

            match &last_turn.response.multi_turn_schema {
                None => vec![
                    last_turn.query.to_string(),
                    last_turn.response.response.to_string(),
                    query.to_string(),
                ],
                // use rephrased query if in-context
                Some(MultiTurnSchema {
                    is_query: true,
                    in_context: true,
                    query: rephrased_query,
                }) => vec![
                    rephrased_query.to_string(),
                    last_turn.response.response.to_string(),
                    query.to_string(),
                ],
                // still in context, but is not a query (e.g. greetings)
                Some(MultiTurnSchema {
                    is_query: false,
                    in_context: true,
                    query: _,
                }) => {
                    let before_last_turn = history.get(history.len() - 2).unwrap();

                    vec![
                        before_last_turn.query.to_string(),
                        before_last_turn.response.response.to_string(),
                        last_turn.query.to_string(),
                        last_turn.response.response.to_string(),
                        query.to_string(),
                    ]
                },
                // start a new context
                Some(MultiTurnSchema {
                    in_context: false,
                    ..
                }) => vec![
                    last_turn.query.to_string(),
                    last_turn.response.response.to_string(),
                    query.to_string(),
                ],
            }
        },
    }
}
