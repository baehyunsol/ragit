use super::FileTree;
use crate::Keywords;
use crate::chunk::{Chunk, RenderedChunk};
use crate::error::Error;
use crate::index::Index;
use crate::query::QueryResponse;
use crate::uid::{Uid, UidQueryConfig};
use ragit_cli::substr_edit_distance;
use ragit_pdl::Schema;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum Action {
    ReadFile,
    ReadDir,
    ReadChunk,
    SearchExact,
    SearchTfidf,

    /// This action will be filtered out if there's no metadata.
    GetMeta,

    /// I'm using the term "simple" because I want to make sure that it's not an agentic RAG.
    SimpleRag,
}

impl Action {
    pub fn all_actions() -> Vec<Action> {
        vec![
            Action::ReadFile,
            Action::ReadDir,
            Action::ReadChunk,
            Action::SearchExact,
            Action::SearchTfidf,
            Action::GetMeta,
            Action::SimpleRag,
        ]
    }

    // If this action requires an argument, the instruction must be "Give me an argument. The argument must be...".
    // If it doesn't require an argument, an AI will always reply "okay" to the instruction (I'll push a fake turn).
    pub(crate) fn get_instruction(&self, index: &Index) -> Result<String, Error> {
        let s = match self {
            Action::ReadFile => String::from("Give me an exact path of a file that you want to read. Don't say anything other than the path of the file."),
            Action::ReadDir => String::from("Give me an exact path of a directory that you want to read. Don't say anything other than the path of the directory. If you want to browse the root directory, just say \"/\"."),
            Action::ReadChunk => String::from("Give me an exact uid of a chunk that you want to read. A uid is a hexadecimal string that uniquely identifies a chunk. Don't say anything other than the uid of the chunk."),
            Action::SearchExact => String::from("Give me a keyword that you want to search for. It's not a pattern, just a keyword (case-sensitive). I'll use exact-text-matching to search. Don't say anything other than the keyword."),
            Action::SearchTfidf => String::from("Give me a comma-separated list of keywords that you want to search for. Don't say anything other than the keywords."),
            Action::GetMeta => format!(
                "Below is a list of keys in the metadata. Choose a key that you want to see. Don't say anything other than the key.\n\n{:?}",
                index.get_all_meta()?.keys().collect::<Vec<_>>(),
            ),
            Action::SimpleRag => String::from("Give me a simple factual question. Don't say anything other than the question."),
        };

        Ok(s)
    }

    // There used to be an action that requires no argument (`Action::GetSummary`),
    // but it's deprecated.
    pub(crate) fn requires_argument(&self) -> bool {
        match self {
            Action::ReadFile => true,
            Action::ReadDir => true,
            Action::ReadChunk => true,
            Action::SearchExact => true,
            Action::SearchTfidf => true,
            Action::GetMeta => true,
            Action::SimpleRag => true,
        }
    }

    pub(crate) fn write_prompt(actions: &[Action]) -> String {
        actions.iter().enumerate().map(
            |(i, p)| format!("{}. {}", i + 1, p.write_unit_prompt())
        ).collect::<Vec<_>>().join("\n")
    }

    pub(crate) fn write_unit_prompt(&self) -> String {
        match self {
            Action::ReadFile => "Read a file: if you give me the exact path of a file, I'll show you the content of the file.",
            Action::ReadDir => "See a list of files in a directory: if you give me the exact path of a directory, I'll show you a list of the files in the directory.",
            Action::ReadChunk => "Read a chunk: if you know a uid of a chunk, you can get the content of the chunk.",
            Action::SearchExact => "Search by a keyword (exact): if you give me a keyword, I'll give you a list of files that contain the exact keyword in their contents.",
            Action::SearchTfidf => "Search by keywords (tfidf): if you give me keywords, I'll give you a tfidf search result. It tries to search for files that contain any of the keywords, even though there's no exact match.",
            Action::GetMeta => "Get metadata: a knowledge-base has metadata, which is a key-value store. If you give me a key of a metadata, I'll give you what value the metadata has.",
            Action::SimpleRag => "Call a simple RAG agent: if you ask a simple factual question, a RAG agent will read the files and answer your question. You can only ask a simple factual question, not complex reasoning questions.",
        }.to_string()
    }

    pub(crate) async fn run(&self, argument: &str, index: &Index) -> Result<ActionResult, Error> {
        let mut argument = argument.trim().to_string();

        // argument is a path
        if let Action::ReadFile | Action::ReadDir = self {
            // If `normalize` fails, that means `argument` is not a valid path,
            // and it will throw an error later.
            argument = ragit_fs::normalize(&argument).unwrap_or(argument);

            if argument.starts_with("/") {
                argument = argument.get(1..).unwrap().to_string();
            }
        }

        let r = match self {
            Action::ReadFile => match index.processed_files.get(&argument) {
                Some(uid) => {
                    let chunk_uids = index.get_chunks_of_file(*uid)?;

                    // If the file is too long, it shows the summaries of its chunks
                    // instead of `cat-file`ing the file.
                    // TODO: what if it's sooooo long that even the chunk list is too long?
                    let max_chunks = index.query_config.max_retrieval;

                    // NOTE: Even an empty file has a chunk. So `.len()` must be greater than 0.
                    match chunk_uids.len() {
                        1 => {
                            let chunk = index.get_chunk_by_uid(chunk_uids[0])?.render(index)?;
                            ActionResult::ReadFileShort {
                                chunk_uids,
                                rendered: chunk,
                            }
                        },
                        n if n <= max_chunks => {
                            let chunk_uids = index.get_chunks_of_file(*uid)?;
                            let chunk = index.get_merged_chunk_of_file(*uid)?;
                            ActionResult::ReadFileShort {
                                chunk_uids,
                                rendered: chunk,
                            }
                        },
                        _ => {
                            let mut chunks = Vec::with_capacity(chunk_uids.len());

                            for chunk_uid in chunk_uids.iter() {
                                chunks.push(index.get_chunk_by_uid(*chunk_uid)?);
                            }

                            ActionResult::ReadFileLong(chunks)
                        },
                    }
                },
                None => {
                    let mut similar_files = vec![];

                    // TODO: it might take very very long time if the knowledge-base is large...
                    for file in index.processed_files.keys() {
                        let dist = substr_edit_distance(argument.as_bytes(), file.as_bytes());

                        if dist < 3 {
                            similar_files.push((file.to_string(), dist));
                        }
                    }

                    similar_files.sort_by_key(|(_, d)| *d);

                    if similar_files.len() > 10 {
                        similar_files = similar_files[..10].to_vec();
                    }

                    let similar_files = similar_files.into_iter().map(|(f, _)| f).collect::<Vec<_>>();
                    ActionResult::NoSuchFile {
                        file: argument.to_string(),
                        similar_files,
                    }
                },
            },
            Action::ReadDir => {
                if !argument.ends_with("/") && argument != "" {
                    argument = format!("{argument}/");
                }

                let mut file_tree = FileTree::root();

                for file in index.processed_files.keys() {
                    if file.starts_with(&argument) {
                        file_tree.insert(file.get(argument.len()..).unwrap());
                    }
                }

                if file_tree.is_empty() {
                    ActionResult::NoSuchDir {
                        dir: argument.to_string(),

                        // TODO: I want to suggest directories with a similar name,
                        //       but it's too tricky to find ones.
                        similar_dirs: vec![],
                    }
                }

                else {
                    ActionResult::ReadDir(file_tree)
                }
            },
            Action::ReadChunk => {
                if !Uid::is_valid_prefix(&argument) {
                    ActionResult::NoSuchChunk(argument.to_string())
                }

                else {
                    let query = index.uid_query(&[argument.to_string()], UidQueryConfig::new().chunk_only())?;
                    let chunk_uids = query.get_chunk_uids();

                    match chunk_uids.len() {
                        0 => ActionResult::NoSuchChunk(argument.to_string()),
                        1 => ActionResult::ReadChunk(index.get_chunk_by_uid(chunk_uids[0])?),
                        2..=10 => {
                            let mut chunks = Vec::with_capacity(chunk_uids.len());

                            for chunk_uid in chunk_uids.iter() {
                                chunks.push(index.get_chunk_by_uid(*chunk_uid)?);
                            }

                            ActionResult::ReadChunkAmbiguous {
                                query: argument.to_string(),
                                chunks,
                            }
                        },
                        _ => ActionResult::ReadChunkTooMany {
                            query: argument.to_string(),
                            chunk_uids: chunk_uids.len(),
                        },
                    }
                }
            },
            Action::SearchExact | Action::SearchTfidf => {
                // The result of exact search is a subset of the result of tfidf search.
                let mut limit = if *self == Action::SearchExact {
                    100
                } else {
                    10
                };

                let chunks = 'chunks_loop: loop {
                    let candidates = index.run_tfidf(
                        Keywords::from_raw(vec![argument.to_string()]),
                        limit,
                    )?;
                    let mut chunks = Vec::with_capacity(candidates.len());
                    let mut chunks_exact_match = vec![];

                    for c in candidates.iter() {
                        chunks.push(index.get_chunk_by_uid(c.id)?);
                    }

                    if *self == Action::SearchTfidf {
                        break chunks;
                    }

                    for chunk in chunks.iter() {
                        if chunk.title.contains(&argument)
                        || chunk.summary.contains(&argument)
                        || chunk.data.contains(&argument)
                        || chunk.render_source().contains(&argument) {
                            chunks_exact_match.push(chunk.clone());

                            if chunks_exact_match.len() == 10 {
                                break 'chunks_loop chunks_exact_match;
                            }
                        }
                    }

                    // We have a complete set of the tfidf result, so there's
                    // no point in increasing the limit.
                    if candidates.len() < limit || limit == index.chunk_count {
                        break chunks_exact_match;
                    }

                    // Maybe we can get more exact-matches if we increase the
                    // limit of the tfidf-match.
                    limit = (limit * 5).min(index.chunk_count);
                };

                ActionResult::Search {
                    r#type: SearchType::from(*self),
                    keyword: argument.to_string(),
                    chunks,
                }
            },
            Action::GetMeta => {
                let mut candidates = vec![
                    argument.to_string(),
                ];

                // small QoL: the AI might wrap the key with quotation marks
                if argument.starts_with("\"") {
                    if let Ok(serde_json::Value::String(s)) = serde_json::from_str(&argument) {
                        candidates.push(s.to_string());
                    }
                }

                let mut result = None;

                for candidate in candidates.iter() {
                    if let Some(value) = index.get_meta_by_key(candidate.to_string())? {
                        result = Some((candidate.to_string(), value));
                        break;
                    }
                }

                if let Some((key, value)) = result {
                    ActionResult::GetMeta {
                        key,
                        value,
                    }
                }

                else {
                    let mut similar_keys = vec![];

                    for key in index.get_all_meta()?.keys() {
                        let dist = substr_edit_distance(argument.as_bytes(), key.as_bytes());

                        if dist < 3 {
                            similar_keys.push((key.to_string(), dist));
                        }
                    }

                    similar_keys.sort_by_key(|(_, d)| *d);

                    if similar_keys.len() > 10 {
                        similar_keys = similar_keys[..10].to_vec();
                    }

                    let similar_keys = similar_keys.into_iter().map(|(f, _)| f).collect::<Vec<_>>();
                    ActionResult::NoSuchMeta {
                        key: argument.to_string(),
                        similar_keys,
                    }
                }
            },
            Action::SimpleRag => {
                let response = index.query(
                    &argument,
                    vec![],  // no history
                    None,  // no output schema
                ).await?;

                ActionResult::SimpleRag(response)
            },
        };

        Ok(r)
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum ActionResult {
    // If the file is short enough, it'll merge its chunks into one.
    ReadFileShort {
        chunk_uids: Vec<Uid>,
        rendered: RenderedChunk,
    },
    ReadFileLong(Vec<Chunk>),
    NoSuchFile {
        file: String,
        similar_files: Vec<String>,
    },

    ReadDir(FileTree),
    NoSuchDir {
        dir: String,
        similar_dirs: Vec<String>,
    },
    ReadChunk(Chunk),
    NoSuchChunk(String),
    ReadChunkAmbiguous {
        query: String,
        chunks: Vec<Chunk>,
    },
    ReadChunkTooMany {
        query: String,
        chunk_uids: usize,
    },
    Search {
        r#type: SearchType,
        keyword: String,
        chunks: Vec<Chunk>,
    },
    GetMeta {
        key: String,
        value: String,
    },
    NoSuchMeta {
        key: String,
        similar_keys: Vec<String>,
    },
    SimpleRag(QueryResponse),
}

impl ActionResult {
    // If it's ok, the AI can update the context with information from `self.render()`.
    // If it's not ok, `self.render()` will instruct the AI how to generate a valid argument.
    pub fn has_to_retry(&self) -> bool {
        match self {
            ActionResult::ReadFileShort { .. }
            | ActionResult::ReadFileLong(_)
            | ActionResult::ReadDir(_)
            | ActionResult::ReadChunk(_)
            | ActionResult::ReadChunkTooMany { .. }  // There's nothing AI can do
            | ActionResult::Search { .. }
            | ActionResult::GetMeta { .. }
            | ActionResult::SimpleRag(_) => false,
            ActionResult::NoSuchFile { .. }
            | ActionResult::NoSuchDir { .. }
            | ActionResult::NoSuchChunk(_)
            | ActionResult::ReadChunkAmbiguous { .. }
            | ActionResult::NoSuchMeta { .. } => true,
        }
    }

    // This is exactly what the AI sees (a turn).
    pub fn render(&self) -> String {
        match self {
            ActionResult::ReadFileShort { rendered, .. } => rendered.human_data.clone(),
            ActionResult::ReadFileLong(chunks) => format!(
                "The file is too long to show you. Instead, I'll show you the summaries of the chunks of the file. You can get the content of the chunks with their uid.\n\n{}",
                chunks.iter().enumerate().map(
                    |(index, chunk)| format!(
                        "{}. {}\nuid: {}\nsummary: {}",
                        index + 1,
                        chunk.render_source(),
                        chunk.uid.abbrev(9),
                        chunk.summary,
                    )
                ).collect::<Vec<_>>().join("\n\n"),
            ),
            ActionResult::NoSuchFile { file, similar_files } => format!(
                "There's no such file: `{file}`{}",
                if !similar_files.is_empty() {
                    format!("\nThere are files with a similar name:\n\n{}", similar_files.join("\n"))
                } else {
                    String::new()
                },
            ),
            ActionResult::ReadDir(file_tree) => file_tree.render(),
            ActionResult::NoSuchDir { dir, similar_dirs } => format!(
                "There's no such dir: `{dir}`{}",
                if !similar_dirs.is_empty() {
                    format!("\nThere are dirs with a similar name:\n\n{}", similar_dirs.join("\n"))
                } else {
                    String::new()
                },
            ),
            ActionResult::ReadChunk(chunk) => chunk.data.clone(),
            ActionResult::NoSuchChunk(query) => {
                if !Uid::is_valid_prefix(&query) {
                    format!("{query:?} is not a valid uid. A uid is a 9 ~ 64 characters long hexadecimal string that uniquely identifies a chunk.")
                }

                else {
                    format!("There's no chunk that has uid `{query}`.")
                }
            },
            ActionResult::ReadChunkAmbiguous { query, chunks } => {
                let chunks = chunks.iter().enumerate().map(
                    |(index, chunk)| format!(
                        "{}. {}\nuid: {}\ntitle: {}\nsummary: {}",
                        index + 1,
                        chunk.render_source(),
                        chunk.uid.abbrev(query.len() + 4),
                        chunk.title,
                        chunk.summary,
                    )
                ).collect::<Vec<_>>().join("\n\n");
                format!("There are multiple chunks whose uid starts with `{query}`. Please give me a longer uid so that I can uniquely identify the chunk.\n\n{chunks}")
            },
            ActionResult::ReadChunkTooMany { query, chunk_uids } => {
                // `Action::ReadChunk`'s default abbrev is 9.
                if query.len() >= 9 {
                    format!("I'm sorry, but you're very unlucky. There're {chunk_uids} chunks whose uid starts with `{query}`. I can't help it.")
                }

                else {
                    format!("Your query `{query}` is too ambiguous. There are {chunk_uids} chunks whose uid starts with `{query}`. Please give me a longer uid so that I can uniquely identify the chunk.")
                }
            },
            ActionResult::Search { r#type, keyword, chunks } => {
                if chunks.is_empty() {
                    match r#type {
                        SearchType::Exact => format!("There's no file that contains the keyword `{keyword}`. Perhaps try tfidf search with the same keyword."),
                        SearchType::Tfidf => format!("There's no file that matches keywords `{keyword}`.")
                    }
                }

                else {
                    let header = format!(
                        "This is a list of chunks that {} `{keyword}`.",
                        match r#type {
                            SearchType::Exact => "contains the keyword",
                            SearchType::Tfidf => "matches keywords",
                        },
                    );

                    format!(
                        "{header}\n\n{}",
                        chunks.iter().enumerate().map(
                            |(index, chunk)| format!(
                                "{}. {}\nuid: {}\nsummary: {}",
                                index + 1,
                                chunk.render_source(),
                                chunk.uid.abbrev(9),
                                chunk.summary,
                            )
                        ).collect::<Vec<_>>().join("\n\n")
                    )
                }
            },
            ActionResult::GetMeta { value, .. } => value.clone(),
            ActionResult::NoSuchMeta { key, similar_keys } => format!(
                "There's no such key in metadata: `{key}`{}",
                if !similar_keys.is_empty() {
                    format!("\nThere are similar keys:\n\n{similar_keys:?}")
                } else {
                    String::new()
                },
            ),
            ActionResult::SimpleRag(response) => format!(
                "{}{}",
                response.response.clone(),
                if response.retrieved_chunks.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n\n---- referenced chunks ----\n{}",
                        response.retrieved_chunks.iter().map(
                            |c| format!("{} (uid: {})", c.render_source(), c.uid.abbrev(9))
                        ).collect::<Vec<_>>().join("\n"),
                    )
                },
            ),
        }
    }
}

// The primary goal of this struct is to render `agent.pdl`.
#[derive(Debug, Default, Serialize)]
pub(crate) struct ActionState {
    // A set of actions that it can run.
    #[serde(skip)]
    pub actions: Vec<Action>,

    // It uses an index of an action instead of the action itself.
    // That's because it's tricky to (de)serialize actions.
    pub index: Option<usize>,
    pub instruction: Option<String>,

    // It might take multiple turns for the AI to generate an argument.
    // e.g. if it tries to read a file that does not exist, the engine
    // will give a feedback and the AI will retry
    pub argument_turns: Vec<ArgumentTurn>,

    // There's a valid argument in `argument_turns`, and it's run.
    pub complete: bool,

    pub result: Option<ActionResult>,

    // If yes, it runs another action within the same context
    pub r#continue: Option<String>,  // "yes" | "no"
}

impl ActionState {
    pub fn new(actions: Vec<Action>) -> Self {
        ActionState {
            actions,
            complete: false,
            ..ActionState::default()
        }
    }

    pub fn get_schema(&self) -> Option<Schema> {
        if self.index.is_none() {
            Some(Schema::integer_between(Some(1), Some(self.actions.len() as i128)))
        }

        else if !self.complete {
            None
        }

        else if self.r#continue.is_none() {
            Some(Schema::default_yesno())
        }

        else {
            unreachable!()
        }
    }

    pub async fn update(
        &mut self,
        input: Value,
        index: &Index,
        action_traces: &mut Vec<ActionTrace>,
    ) -> Result<(), Error> {
        if self.index.is_none() {
            // If `input.as_u64()` fails, that means the AI is so stupid
            // that it cannot choose a number even with pdl schema's help.
            // So we just choose an arbitrary action. The AI's gonna fail
            // anyway and will break soon.
            let n = input.as_u64().unwrap_or(1) as usize;
            let action = self.actions[n - 1];  // AI uses 1-based index
            self.index = Some(n);
            self.instruction = Some(action.get_instruction(index)?);

            if !action.requires_argument() {
                // See comments in `Action::get_instruction`
                let argument = "okay";
                let result = action.run("", index).await?;
                let mut result_rendered = result.render();

                if !result.has_to_retry() {
                    self.complete = true;
                }

                // If it's not complete, we have to give the instruction again so that the AI
                // will generate the argument.
                else {
                    result_rendered = format!("{result_rendered}\n\n{}", action.get_instruction(index)?);
                }

                self.argument_turns.push(ArgumentTurn {
                    assistant: argument.to_string(),
                    user: result_rendered.to_string(),
                });
                self.result = Some(result.clone());
                action_traces.push(ActionTrace {
                    action,
                    argument: None,
                    result: result.clone(),
                });
            }
        }

        else if !self.complete {
            let action = self.actions[self.index.unwrap() - 1];  // AI uses 1-based index

            // NOTE: pdl schema `string` is infallible
            let argument = input.as_str().unwrap();
            let result = action.run(argument, index).await?;
            let mut result_rendered = result.render();

            // Some AIs are not smart enough to generate a valid argument.
            // If the AI fails to generate valid argument more than once,
            // it just breaks.
            if !result.has_to_retry() || self.argument_turns.len() > 0 {
                self.complete = true;
            }

            else {
                result_rendered = format!("{result_rendered}\n\n{}", action.get_instruction(index)?);
            }

            self.argument_turns.push(ArgumentTurn {
                assistant: argument.to_string(),
                user: result_rendered.to_string(),
            });
            self.result = Some(result.clone());
            action_traces.push(ActionTrace {
                action,
                argument: None,
                result: result.clone(),
            });
        }

        else if self.r#continue.is_none() {
            // If `input.as_bool()` fails, that means the AI is
            // not smart enough to generate a boolean. There's
            // no need to continue.
            let input = input.as_bool().unwrap_or(false);
            let s = if input { "yes" } else { "no" };

            self.r#continue = Some(s.to_string());
        }

        else {
            unreachable!()
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ArgumentTurn {
    // An argument of an action, that AI generated
    assistant: String,

    // If the argument is valid, it's a result of the action.
    // Otherwise, it's a feedback: why the argument is invalid and how to fix it.
    user: String,
}

#[derive(Serialize)]
pub struct ActionTrace {
    pub action: Action,
    pub argument: Option<String>,
    pub result: ActionResult,
}

#[derive(Clone, Debug, Serialize)]
pub enum SearchType {
    Exact,
    Tfidf,
}

impl From<Action> for SearchType {
    fn from(a: Action) -> SearchType {
        match a {
            Action::SearchExact => SearchType::Exact,
            Action::SearchTfidf => SearchType::Tfidf,
            _ => panic!(),
        }
    }
}
