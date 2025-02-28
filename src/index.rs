use crate::api_config::{ApiConfig, ApiConfigRaw};
use crate::chunk::{self, Chunk, ChunkBuildInfo};
use crate::constant::{
    API_CONFIG_FILE_NAME,
    BUILD_CONFIG_FILE_NAME,
    CHUNK_DIR_NAME,
    CONFIG_DIR_NAME,
    FILE_INDEX_DIR_NAME,
    II_DIR_NAME,
    IMAGE_DIR_NAME,
    INDEX_DIR_NAME,
    INDEX_FILE_NAME,
    LOG_DIR_NAME,
    MODEL_FILE_NAME,
    PROMPT_DIR_NAME,
    QUERY_CONFIG_FILE_NAME,
};
use crate::error::Error;
use crate::prompts::PROMPTS;
use crate::query::{Keywords, QueryConfig};
use crate::uid::{self, Uid, UidWriteMode};
use ragit_api::{
    Model,
    ModelRaw,
    RecordAt,
    Request,
};
use ragit_fs::{
    WriteMode,
    create_dir_all,
    exists,
    extension,
    into_abs_path,
    is_dir,
    join,
    join3,
    join4,
    normalize,
    parent,
    read_bytes,
    read_dir,
    read_string,
    remove_file,
    set_extension,
    try_create_dir,
    write_bytes,
    write_string,
};
use ragit_pdl::{
    Pdl,
    encode_base64,
    parse_pdl,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

mod auth;
mod commands;
mod config;
pub mod file;
mod ii;
pub mod tfidf;

pub use commands::{
    AddMode,
    AddResult,
    MergeMode,
    MergeResult,
    RecoverResult,
    RemoveResult,
    VersionInfo,
    get_compatibility_warning,
};
pub use config::BuildConfig;
pub use file::{FileReader, ImageDescription};
pub use ii::IIStatus;
pub use tfidf::{ProcessedDoc, TfidfResult, TfidfState, consume_processed_doc};

pub type Path = String;

// all the `Path` are normalized relative paths
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Index {
    ragit_version: String,
    pub chunk_count: usize,
    pub staged_files: Vec<Path>,
    pub processed_files: HashMap<Path, Uid>,

    // Previously, all the builds were in serial and this field tells
    // which file the index is building. When something goes wrong, ragit
    // reads this field and clean up garbages. Now, all the builds are in
    // parallel and there's no such thing like `curr_processing_file`. But
    // we still need to tell whether something went wrong while building
    // and this field does that. If it's `Some(_)`, something's wrong and
    // clean-up has to be done.
    pub curr_processing_file: Option<Path>,

    // TODO: we should not use the term `repo` in this repo
    repo_url: Option<String>,

    /// `ii` stands for `inverted-index`.
    pub ii_status: IIStatus,

    // it's not used by code, but used by serde
    // users modify json file, which is deserialized to `ApiConfigRaw`,
    // which is then converted to `ApiConfig` by `.init_api_config()`
    #[serde(skip)]
    api_config_raw: ApiConfigRaw,

    #[serde(skip)]
    pub root_dir: Path,
    #[serde(skip)]
    pub build_config: BuildConfig,
    #[serde(skip)]
    pub query_config: QueryConfig,
    #[serde(skip)]
    pub api_config: ApiConfig,
    #[serde(skip)]
    pub prompts: HashMap<String, String>,
    #[serde(skip)]
    pub models: Vec<Model>,
}

/// 1. If you want to do something with chunks, use `LoadMode::QuickCheck`.
/// 2. If you have nothing to do with chunks, use `LoadMode::OnlyJson`.
/// 3. If something's broken and you don't want it to crash, use `LoadMode::Minimum`. It can still crash, though.
/// 4. If you want to be very sure that nothing's broken and you don't care about init-time, use `LoadMode::Check`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LoadMode {
    /// It only loads `index.json`. It doesn't care whether config files prompt files, or chunk files are broken.
    Minimum,

    /// It loads json files, but doesn't care whether the chunk files are broken.
    OnlyJson,

    /// It checks and auto-recovers if `self.curr_processing_file` is not None. If the value is not None,
    /// a previous build was interrupted and something could be broken.
    QuickCheck,

    /// It always checks and auto-recovers. You should be very careful, `check` and `auto-recover` are very expensive.
    Check,
}

impl Index {
    pub fn dummy() -> Self {
        Index {
            ragit_version: String::new(),
            chunk_count: 0,
            staged_files: vec![],
            processed_files: HashMap::new(),
            curr_processing_file: None,
            repo_url: None,
            ii_status: IIStatus::None,
            api_config_raw: ApiConfigRaw::default(),
            root_dir: String::from("."),
            build_config: BuildConfig::default(),
            query_config: QueryConfig::default(),
            api_config: ApiConfig::default(),
            prompts: HashMap::new(),
            models: vec![],
        }
    }

    /// It works like git. `root_dir` is the root of the repo. And it creates dir `.ragit/`, like `.git/`.
    /// It reads the files in the repo and creates index.
    pub fn new(
        root_dir: Path,
    ) -> Result<Self, Error> {
        let index_dir = join(&root_dir, INDEX_DIR_NAME)?;
        let root_dir = normalize(&into_abs_path(&root_dir)?)?;

        if exists(&index_dir) {
            return Err(Error::IndexAlreadyExists(index_dir));
        }

        create_dir_all(&index_dir)?;

        for dir in [
            CONFIG_DIR_NAME,
            CHUNK_DIR_NAME,
            IMAGE_DIR_NAME,
            FILE_INDEX_DIR_NAME,
            II_DIR_NAME,
        ] {
            create_dir_all(&Index::get_rag_path(
                &root_dir,
                &dir.to_string(),
            )?)?;
        }

        // Start with default configs
        let mut build_config = BuildConfig::default();
        let mut query_config = QueryConfig::default();
        let api_config = ApiConfig::default();
        
        // Create a temporary Index to use for loading configs from home
        let temp_index = Index {
            ragit_version: crate::VERSION.to_string(),
            chunk_count: 0,
            staged_files: vec![],
            processed_files: HashMap::new(),
            curr_processing_file: None,
            build_config: build_config.clone(),
            query_config: query_config.clone(),
            api_config_raw: ApiConfigRaw::default(),
            api_config: api_config.clone(),
            root_dir: root_dir.clone(),
            repo_url: None,
            ii_status: IIStatus::None,
            prompts: PROMPTS.clone(),
            models: vec![],
        };
        
        // Try to load build config from home directory and apply to defaults
        if let Ok(Some(partial_build_config)) = temp_index.load_build_config_from_home() {
            // Apply partial config to the default config
            partial_build_config.apply_to(&mut build_config);
        }
        
        // Try to load query config from home directory and apply to defaults
        if let Ok(Some(partial_query_config)) = temp_index.load_query_config_from_home() {
            // Apply partial config to the default config
            partial_query_config.apply_to(&mut query_config);
        }
        
        let mut result = Index {
            ragit_version: crate::VERSION.to_string(),
            chunk_count: 0,
            staged_files: vec![],
            processed_files: HashMap::new(),
            curr_processing_file: None,
            build_config,
            query_config,
            api_config_raw: ApiConfigRaw::default(), // Temporary default, will be updated after loading models
            api_config,
            root_dir,
            repo_url: None,
            ii_status: IIStatus::None,
            prompts: PROMPTS.clone(),
            models: vec![],
        };

        // Load models first so we can choose an appropriate default model
        result.load_or_init_models()?;
        
        // Now update api_config_raw with a valid model
        result.api_config_raw = result.get_default_api_config_raw()?;
        write_bytes(
            &result.get_build_config_path()?,
            &serde_json::to_vec_pretty(&result.build_config)?,
            WriteMode::AlwaysCreate,
        )?;
        write_bytes(
            &result.get_query_config_path()?,
            &serde_json::to_vec_pretty(&result.query_config)?,
            WriteMode::AlwaysCreate,
        )?;
        write_bytes(
            &result.get_api_config_path()?,
            &serde_json::to_vec_pretty(&result.api_config_raw)?,
            WriteMode::AlwaysCreate,
        )?;
        result.save_to_file()?;

        Ok(result)
    }

    pub fn load(
        root_dir: Path,
        load_mode: LoadMode,
    ) -> Result<Self, Error> {
        let mut result = Index::load_minimum(root_dir)?;

        if load_mode == LoadMode::Minimum {
            return Ok(result);
        }

        result.build_config = serde_json::from_str::<BuildConfig>(
            &read_string(&result.get_build_config_path()?)?,
        )?;
        result.query_config = serde_json::from_str::<QueryConfig>(
            &read_string(&result.get_query_config_path()?)?,
        )?;
        result.api_config_raw = serde_json::from_str::<ApiConfigRaw>(
            &read_string(&result.get_api_config_path()?)?,
        )?;
        
        // Load models before initializing API config to ensure we can validate the model
        result.load_or_init_prompts()?;
        result.load_or_init_models()?;
        
        // Check if the model in api_config_raw exists in the loaded models
        let model_exists = ragit_api::get_model_by_name(&result.models, &result.api_config_raw.model).is_ok();
        
        if !model_exists && !result.models.is_empty() {
            // Find the lowest-cost model and update api_config_raw
            if let Some(lowest_cost_model) = result.find_lowest_cost_model() {
                eprintln!("Warning: Model '{}' not found in models.json. Using lowest-cost model '{}' instead.", 
                         result.api_config_raw.model, lowest_cost_model.name);
                
                // Update the model in the config
                result.api_config_raw.model = lowest_cost_model.name.clone();
                
                // Save the updated config
                write_bytes(
                    &result.get_api_config_path()?,
                    &serde_json::to_vec_pretty(&result.api_config_raw)?,
                    WriteMode::Atomic,
                )?;
            }
        }
        
        // Now initialize the API config with the validated model
        result.api_config = result.init_api_config(&result.api_config_raw)?;

        match load_mode {
            LoadMode::QuickCheck if result.curr_processing_file.is_some() => {
                result.recover()?;
                Ok(result)
            },
            LoadMode::Check if result.curr_processing_file.is_some() || result.check().is_err() => {
                result.recover()?;
                Ok(result)
            },
            _ => Ok(result),
        }
    }

    /// It only loads `index.json`. No config files, no prompt files, and it doesn't care whether chunk files are broken or not.
    /// It's for `rag check --recover`: it only loads minimum data and the recover function will load or fix the others.
    fn load_minimum(root_dir: Path) -> Result<Self, Error> {
        let root_dir = normalize(&into_abs_path(&root_dir)?)?;
        let index_json = read_string(&Index::get_rag_path(
            &root_dir,
            &INDEX_FILE_NAME.to_string(),
        )?)?;

        let mut result = serde_json::from_str::<Index>(&index_json)?;
        result.root_dir = root_dir;

        if let Some(warn) = get_compatibility_warning(&result.ragit_version, crate::VERSION) {
            eprintln!("Warning: {warn}");
        }

        Ok(result)
    }

    pub fn load_or_init(
        root_dir: Path,
    ) -> Result<Self, Error> {
        let index_dir = join(
            &root_dir,
            &INDEX_DIR_NAME.to_string(),
        )?;

        if exists(&index_dir) {
            // `load_or_init` cannot be done in only-json mode, because only-json `init` doesn't make sense
            Index::load(root_dir, LoadMode::QuickCheck)
        }

        else {
            Index::new(root_dir)
        }
    }

    pub fn save_to_file(&self) -> Result<(), Error> {
        self.save_prompts()?;

        Ok(write_bytes(
            &Index::get_rag_path(
                &self.root_dir,
                &INDEX_FILE_NAME.to_string(),
            )?,
            &serde_json::to_vec_pretty(self)?,
            WriteMode::Atomic,
        )?)
    }

    pub(crate) async fn load_chunks_or_tfidf(
        &self,
        query: &str,
    ) -> Result<Vec<Chunk>, Error> {
        if self.chunk_count > self.query_config.max_titles {
            let keywords = self.extract_keywords(query).await?;
            let tfidf_results = self.run_tfidf(
                keywords,
                self.query_config.max_summaries,
            )?;
            let mut chunks = Vec::with_capacity(tfidf_results.len());

            for tfidf_result in tfidf_results.into_iter() {
                let uid = tfidf_result.id;
                chunks.push(self.get_chunk_by_uid(uid)?);
            }

            Ok(chunks)
        }

        else {
            let mut chunks = vec![];

            for chunk_path in self.get_all_chunk_files()? {
                chunks.push(chunk::load_from_file(&chunk_path)?);
            }

            Ok(chunks)
        }
    }

    pub fn get_all_chunk_files(&self) -> Result<Vec<Path>, Error> {
        let mut result = vec![];

        for internal in read_dir(&join3(&self.root_dir, &INDEX_DIR_NAME, &CHUNK_DIR_NAME)?, false)? {
            if !is_dir(&internal) {
                continue;
            }

            for chunk_file in read_dir(&internal, false)? {
                if extension(&chunk_file).unwrap_or(None).unwrap_or(String::new()) == "chunk" {
                    result.push(chunk_file.to_string());
                }
            }
        }

        // the result has to be deterministic
        result.sort();
        Ok(result)
    }

    pub fn get_all_tfidf_files(&self) -> Result<Vec<Path>, Error> {
        let mut result = vec![];

        for internal in read_dir(&join3(&self.root_dir, &INDEX_DIR_NAME, &CHUNK_DIR_NAME)?, false)? {
            if !is_dir(&internal) {
                continue;
            }

            for tfidf_file in read_dir(&internal, false)? {
                if extension(&tfidf_file).unwrap_or(None).unwrap_or(String::new()) == "tfidf" {
                    result.push(tfidf_file.to_string());
                }
            }
        }

        // the result has to be deterministic
        result.sort();
        Ok(result)
    }

    pub fn get_all_image_files(&self) -> Result<Vec<Path>, Error> {
        let mut result = vec![];

        for internal in read_dir(&join3(&self.root_dir, &INDEX_DIR_NAME, &IMAGE_DIR_NAME)?, false)? {
            if !is_dir(&internal) {
                continue;
            }

            for image_file in read_dir(&internal, false)? {
                if extension(&image_file).unwrap_or(None).unwrap_or(String::new()) == "png" {
                    result.push(image_file.to_string());
                }
            }
        }

        // the result has to be deterministic
        result.sort();
        Ok(result)
    }

    fn get_all_file_indexes(&self) -> Result<Vec<Path>, Error> {
        let mut result = vec![];

        for internal in read_dir(&join3(&self.root_dir, &INDEX_DIR_NAME, &FILE_INDEX_DIR_NAME)?, false)? {
            if !is_dir(&internal) {
                continue;
            }

            for file_index in read_dir(&internal, false)? {
                result.push(file_index.to_string());
            }
        }

        // the result has to be deterministic
        result.sort();
        Ok(result)
    }

    async fn add_image_description(&self, uid: Uid) -> Result<(), Error> {
        let description_path = Index::get_uid_path(
            &self.root_dir,
            IMAGE_DIR_NAME,
            uid,
            Some("json"),
        )?;
        let image_path = Index::get_uid_path(
            &self.root_dir,
            IMAGE_DIR_NAME,
            uid,
            Some("png"),
        )?;
        let parent_path = parent(&image_path)?;

        if !exists(&parent_path) {
            try_create_dir(&parent_path)?;
        }

        let image_bytes = read_bytes(&image_path)?;
        let image_bytes = encode_base64(&image_bytes);

        if let Ok(j) = read_string(&description_path) {
            if serde_json::from_str::<Value>(&j).is_ok() {
                return Ok(());
            }

            else {
                remove_file(&description_path)?;
            }
        }

        let mut context = tera::Context::new();
        context.insert("image_type", "png");
        context.insert("image_bytes", &image_bytes);
        let pdl = self.get_prompt("describe_image")?;
        let Pdl { messages, schema } = parse_pdl(
            &pdl,
            &context,
            "/",  // TODO: `<|media|>` is not supported for this prompt
            true,
            true,
        )?;

        let request = Request {
            messages,
            model: self.get_model_by_name(&self.api_config.model)?,
            frequency_penalty: None,
            max_tokens: None,
            max_retry: self.api_config.max_retry,
            sleep_between_retries: self.api_config.sleep_between_retries,
            timeout: self.api_config.timeout,
            temperature: None,
            record_api_usage_at: self.api_config.dump_api_usage_at.clone().map(
                |path| RecordAt { path, id: String::from("describe_image") }
            ),
            dump_pdl_at: self.api_config.create_pdl_path("describe_image"),
            dump_json_at: self.api_config.dump_log_at.clone(),
            schema,
            schema_max_try: 3,
        };
        let result = request.send_and_validate::<ImageDescription>(ImageDescription::default()).await?;

        write_bytes(
            &description_path,
            &serde_json::to_vec_pretty(&result)?,
            WriteMode::Atomic,
        )?;

        Ok(())
    }

    pub fn run_tfidf(
        &self,
        keywords: Keywords,
        limit: usize,
    ) -> Result<Vec<TfidfResult<Uid>>, Error> {
        let mut tfidf_state = TfidfState::new(&keywords);

        // TODO: it must be configurable, or at least do much more experiment on this
        let ii_coeff = 20;

        if self.query_config.enable_ii && self.is_ii_built() {
            for chunk_uid in self.get_search_candidates(
                &tfidf_state.terms,
                limit * ii_coeff,
            )? {
                let processed_doc = self.get_tfidf_by_chunk_uid(chunk_uid)?;
                consume_processed_doc(
                    processed_doc,
                    &mut tfidf_state,
                )?;
            }
        }

        else {
            for tfidf_file in self.get_all_tfidf_files()? {
                let processed_doc = tfidf::load_from_file(&tfidf_file)?;
                consume_processed_doc(
                    processed_doc,
                    &mut tfidf_state,
                )?;
            }
        }

        Ok(tfidf_state.get_top(limit))
    }

    pub fn get_chunk_by_uid(&self, uid: Uid) -> Result<Chunk, Error> {
        let chunk_at = Index::get_uid_path(
            &self.root_dir,
            CHUNK_DIR_NAME,
            uid,
            Some("chunk"),
        )?;

        if exists(&chunk_at) {
            return Ok(chunk::load_from_file(&chunk_at)?);
        }

        Err(Error::NoSuchChunk(uid))
    }

    pub fn check_chunk_by_uid(&self, uid: Uid) -> bool {
        if let Ok(chunk_at) = Index::get_uid_path(
            &self.root_dir,
            CHUNK_DIR_NAME,
            uid,
            Some("chunk"),
        ) {
            exists(&chunk_at)
        }

        else {
            false
        }
    }

    pub fn get_tfidf_by_chunk_uid(
        &self,
        uid: Uid,
    ) -> Result<ProcessedDoc, Error> {
        let tfidf_at = Index::get_uid_path(
            &self.root_dir,
            CHUNK_DIR_NAME,
            uid,
            Some("tfidf"),
        )?;

        if exists(&tfidf_at) {
            return Ok(tfidf::load_from_file(&tfidf_at)?);
        }

        Err(Error::NoSuchChunk(uid))
    }

    pub fn get_tfidf_by_file_uid(
        &self,
        uid: Uid,
    ) -> Result<ProcessedDoc, Error> {
        let chunk_uids = self.get_chunks_of_file(uid)?;
        let mut result = ProcessedDoc::empty();

        for uid in chunk_uids.iter() {
            result.extend(&self.get_tfidf_by_chunk_uid(*uid)?);
        }

        result.uid = Some(uid);
        Ok(result)
    }

    // every path in index.json are relative path to root_dir
    fn get_rag_path(root_dir: &Path, rel_path: &Path) -> Result<Path, Error> {
        Ok(normalize(
            &join3(
                root_dir,
                &INDEX_DIR_NAME.to_string(),
                rel_path,
            )?,
        )?)
    }

    pub(crate) fn get_data_path(root_dir: &Path, rel_path: &Path) -> Result<Path, Error> {
        Ok(normalize(
            &join(
                root_dir,
                rel_path,
            )?,
        )?)
    }

    /// `{root_dir}/.ragit/{dir}/uid_prefix/uid_suffix(.{ext})?`
    pub(crate) fn get_uid_path(root_dir: &str, dir: &str, uid: Uid, ext: Option<&str>) -> Result<Path, Error> {
        let dir = join3(
            root_dir,
            INDEX_DIR_NAME,
            dir,
        )?;
        let uid_prefix = uid.get_prefix();
        let uid_suffix = uid.get_suffix();

        let mut result = join3(
            &dir,
            &uid_prefix,
            &uid_suffix,
        )?;

        if let Some(ext) = ext {
            result = set_extension(&result, ext)?;
        }

        Ok(result)
    }

    // root_dir/.ragit/ii/term_hash_prefix/term_hash_suffix
    fn get_ii_path(root_dir: &str, term_hash: String) -> Path {
        let ii_at = join3(
            root_dir,
            &INDEX_DIR_NAME,
            &II_DIR_NAME,
        ).unwrap();
        let term_hash_prefix = term_hash.get(0..2).unwrap().to_string();
        let term_hash_suffix = term_hash.get(2..).unwrap().to_string();

        join3(
            &ii_at,
            &term_hash_prefix,
            &term_hash_suffix,
        ).unwrap()
    }

    fn get_api_config_path(&self) -> Result<Path, Error> {
        Ok(Index::get_rag_path(
            &self.root_dir,
            &join(
                CONFIG_DIR_NAME,
                API_CONFIG_FILE_NAME,
            )?,
        )?)
    }

    fn get_build_config_path(&self) -> Result<Path, Error> {
        Ok(Index::get_rag_path(
            &self.root_dir,
            &join(
                CONFIG_DIR_NAME,
                BUILD_CONFIG_FILE_NAME,
            )?,
        )?)
    }

    fn get_query_config_path(&self) -> Result<Path, Error> {
        Ok(Index::get_rag_path(
            &self.root_dir,
            &join(
                CONFIG_DIR_NAME,
                QUERY_CONFIG_FILE_NAME,
            )?,
        )?)
    }

    pub(crate) fn init_api_config(&self, raw: &ApiConfigRaw) -> Result<ApiConfig, Error> {
        let dump_log_at = if raw.dump_log {
            let path = Index::get_rag_path(
                &self.root_dir,
                &LOG_DIR_NAME.to_string(),
            )?;

            if !exists(&path) || !is_dir(&path) {
                create_dir_all(&path)?;
            }

            Some(path)
        }

        else {
            None
        };

        let dump_api_usage_at = if raw.dump_api_usage {
            let path = Index::get_rag_path(
                &self.root_dir,
                &"usages.json".to_string(),
            )?;

            if !exists(&path) || is_dir(&path) {
                write_string(
                    &path,
                    "{}",
                    WriteMode::AlwaysCreate,
                )?;
            }

            Some(path)
        }

        else {
            None
        };

        Ok(ApiConfig {
            api_key: raw.api_key.clone(),
            max_retry: raw.max_retry,
            model: raw.model.clone(),
            timeout: raw.timeout,
            sleep_after_llm_call: raw.sleep_after_llm_call,
            sleep_between_retries: raw.sleep_between_retries,
            dump_log_at,
            dump_api_usage_at,
        })
    }

    // `Index::load` calls this function. There's no need to call this again.
    pub(crate) fn load_or_init_prompts(&mut self) -> Result<(), Error> {
        let mut has_inited_prompt = false;

        for prompt_name in PROMPTS.keys() {
            let prompt_path = Index::get_rag_path(
                &self.root_dir,
                &join(
                    PROMPT_DIR_NAME,
                    &set_extension(
                        prompt_name,
                        "pdl",
                    )?,
                )?,
            )?;

            match read_string(&prompt_path) {
                Ok(p) => {
                    self.prompts.insert(prompt_name.to_string(), p);
                },
                Err(_) => {
                    eprintln!("Warning: failed to load `{prompt_name}.pdl`");
                    self.prompts.insert(prompt_name.to_string(), PROMPTS.get(prompt_name).unwrap().to_string());
                    has_inited_prompt = true;
                },
            }
        }

        if has_inited_prompt {
            self.save_prompts()?;
        }

        Ok(())
    }

    pub fn save_prompts(&self) -> Result<(), Error> {
        let prompt_real_dir = Index::get_rag_path(
            &self.root_dir,
            &PROMPT_DIR_NAME.to_string(),
        )?;

        if !exists(&prompt_real_dir) {
            create_dir_all(&prompt_real_dir)?;
        }

        // TODO: what if `prompt_name` has `"/"` in it?
        for (prompt_name, prompt) in self.prompts.iter() {
            let prompt_path = join(
                &prompt_real_dir,
                &set_extension(
                    prompt_name,
                    "pdl",
                )?,
            )?;

            write_string(
                &prompt_path,
                prompt,
                WriteMode::Atomic,
            )?;
        }

        Ok(())
    }

    /// It does NOT save the prompt to the file. You have to run `save_prompts` to save it.
    /// `key` is a name of the prompt, like `extract_keyword`, not `extract_keyword.pdl`.
    /// `value` is a content of a pdl file.
    pub fn update_prompt(&mut self, key: String, value: String) {
        self.prompts.insert(key, value);
    }

    pub(crate) fn load_or_init_models(&mut self) -> Result<(), Error> {
        let models_at = Index::get_rag_path(
            &self.root_dir,
            &MODEL_FILE_NAME.to_string(),
        )?;

        if !exists(&models_at) {
            // Initialize models from an external source or defaults
            let models = self.get_initial_models()?;
            
            // Always ensure API keys are null in the local models file
            let models_without_api_keys = self.remove_api_keys_from_models(models);
            
            // Write the models to the local file
            write_string(
                &models_at,
                &serde_json::to_string_pretty(&models_without_api_keys)?,
                WriteMode::Atomic,
            )?;
        }

        // Load models from the local file
        let j = read_string(&models_at)?;
        let models = serde_json::from_str::<Vec<ModelRaw>>(&j)?;
        let mut result = vec![];

        for model in models.iter() {
            result.push(Model::try_from(model)?);
        }

        self.models = result;
        Ok(())
    }
    
    // Get initial models from environment variable, config file, or defaults
    fn get_initial_models(&self) -> Result<Vec<ModelRaw>, Error> {
        // Check for environment variable RAGIT_MODEL_CONFIG
        if let Ok(env_path) = std::env::var("RAGIT_MODEL_CONFIG") {
            if exists(&env_path) {
                // Load from the environment variable path
                let env_content = read_string(&env_path)?;
                if let Ok(models) = serde_json::from_str::<Vec<ModelRaw>>(&env_content) {
                    return Ok(models);
                } else {
                    eprintln!("Warning: Could not parse models from RAGIT_MODEL_CONFIG, falling back to defaults");
                }
            } else {
                eprintln!("Warning: RAGIT_MODEL_CONFIG points to non-existent file: {}", env_path);
            }
        }
        
        // Check for ~/.config/ragit/models.json
        let home_dir = match std::env::var("HOME") {
            Ok(path) => path,
            Err(_) => {
                eprintln!("Warning: HOME environment variable not set, cannot check ~/.config/ragit/models.json");
                String::new()
            }
        };
        
        if !home_dir.is_empty() {
            let config_path = join4(&home_dir, ".config", "ragit", "models.json")?;
            if exists(&config_path) {
                // Load from ~/.config/ragit/models.json
                let config_content = read_string(&config_path)?;
                if let Ok(models) = serde_json::from_str::<Vec<ModelRaw>>(&config_content) {
                    return Ok(models);
                } else {
                    eprintln!("Warning: Could not parse models from ~/.config/ragit/models.json, falling back to defaults");
                }
            }
        }
        
        // Fall back to default models
        Ok(ModelRaw::default_models())
    }
    
    // Remove API keys from models to ensure they're not stored in the local file
    fn remove_api_keys_from_models(&self, models: Vec<ModelRaw>) -> Vec<ModelRaw> {
        models.into_iter().map(|model| {
            // First convert ModelRaw to Model
            if let Ok(mut model_obj) = Model::try_from(&model) {
                // Create a new Model with api_key set to None
                model_obj.api_key = None;
                // Convert back to ModelRaw
                ModelRaw::from(&model_obj)
            } else {
                // If conversion fails, return the original model
                // This shouldn't happen in practice
                model
            }
        }).collect()
    }

    pub(crate) fn get_model_by_name(&self, name: &str) -> Result<Model, Error> {
        Ok(ragit_api::get_model_by_name(&self.models, name)?.clone())
    }

    pub fn get_prompt(&self, prompt_name: &str) -> Result<String, Error> {
        match self.prompts.get(prompt_name) {
            Some(prompt) => Ok(prompt.to_string()),
            None => Err(Error::PromptMissing(prompt_name.to_string())),
        }
    }

    fn add_file_index(&mut self, file_uid: Uid, uids: &[Uid]) -> Result<(), Error> {
        let file_index_path = Index::get_uid_path(
            &self.root_dir,
            FILE_INDEX_DIR_NAME,
            file_uid,
            None,
        )?;
        let parent_path = parent(&file_index_path)?;

        if !exists(&parent_path) {
            try_create_dir(&parent_path)?;
        }

        uid::save_to_file(&file_index_path, uids, UidWriteMode::Naive)
    }

    fn remove_file_index(&mut self, file_uid: Uid) -> Result<(), Error> {
        let file_index_path = Index::get_uid_path(
            &self.root_dir,
            FILE_INDEX_DIR_NAME,
            file_uid,
            None,
        )?;

        if !exists(&file_index_path) {
            return Err(Error::NoSuchFile { path: None, uid: Some(file_uid) });
        }

        Ok(remove_file(&file_index_path)?)
    }

    pub fn get_chunks_of_file(&self, file_uid: Uid) -> Result<Vec<Uid>, Error> {
        let file_index_path = Index::get_uid_path(
            &self.root_dir,
            FILE_INDEX_DIR_NAME,
            file_uid,
            None,
        )?;

        if exists(&file_index_path) {
            return Ok(uid::load_from_file(&file_index_path)?);
        }

        Err(Error::NoSuchFile { path: None, uid: Some(file_uid) })
    }

    pub fn get_images_of_file(&self, file_uid: Uid) -> Result<Vec<Uid>, Error> {
        let chunk_uids = self.get_chunks_of_file(file_uid)?;
        let mut result = HashSet::new();

        for chunk_uid in chunk_uids.into_iter() {
            let chunk = self.get_chunk_by_uid(chunk_uid)?;

            for image in chunk.images.iter() {
                result.insert(*image);
            }
        }

        Ok(result.into_iter().collect())
    }

    pub fn get_image_bytes_by_uid(&self, uid: Uid) -> Result<Vec<u8>, Error> {
        Ok(read_bytes(&Index::get_uid_path(&self.root_dir, IMAGE_DIR_NAME, uid, Some("png"))?)?)
    }

    pub fn get_image_description_by_uid(&self, uid: Uid) -> Result<ImageDescription, Error> {
        let j = read_string(&Index::get_uid_path(&self.root_dir, IMAGE_DIR_NAME, uid, Some("json"))?)?;
        let v = serde_json::from_str::<ImageDescription>(&j)?;
        Ok(v)
    }

    /// Finds the lowest-cost model in the loaded models.
    fn find_lowest_cost_model(&self) -> Option<&Model> {
        if self.models.is_empty() {
            return None;
        }
        
        self.models.iter()
            .min_by(|a, b| {
                let a_cost = a.dollars_per_1b_input_tokens as u128 + a.dollars_per_1b_output_tokens as u128;
                let b_cost = b.dollars_per_1b_input_tokens as u128 + b.dollars_per_1b_output_tokens as u128;
                a_cost.cmp(&b_cost)
            })
    }
    
    /// Attempts to load a config file from ~/.config/ragit/
    fn load_config_from_home<T: serde::de::DeserializeOwned>(&self, filename: &str) -> Result<Option<T>, Error> {
        // Check for HOME environment variable
        let home_dir = match std::env::var("HOME") {
            Ok(path) => path,
            Err(_) => {
                eprintln!("Warning: HOME environment variable not set, cannot check ~/.config/ragit/{}", filename);
                return Ok(None);
            }
        };
        
        let config_path = join4(&home_dir, ".config", "ragit", filename)?;
        if exists(&config_path) {
            // Load from ~/.config/ragit/filename
            let config_content = read_string(&config_path)?;
            match serde_json::from_str::<T>(&config_content) {
                Ok(config) => {
                    eprintln!("Info: Using configuration from ~/.config/ragit/{}", filename);
                    return Ok(Some(config));
                },
                Err(e) => {
                    eprintln!("Warning: Could not parse {} from ~/.config/ragit/{}: {}", filename, filename, e);
                }
            }
        }
        
        Ok(None)
    }

    /// Attempts to load ApiConfigRaw from ~/.config/ragit/api.json
    fn load_api_config_from_home(&self) -> Result<Option<ApiConfigRaw>, Error> {
        self.load_config_from_home("api.json")
    }
    
    /// Attempts to load PartialQueryConfig from ~/.config/ragit/query.json
    fn load_query_config_from_home(&self) -> Result<Option<crate::query::config::PartialQueryConfig>, Error> {
        self.load_config_from_home("query.json")
    }
    
    /// Attempts to load PartialBuildConfig from ~/.config/ragit/build.json
    fn load_build_config_from_home(&self) -> Result<Option<crate::index::config::PartialBuildConfig>, Error> {
        self.load_config_from_home("build.json")
    }

    /// Returns a default ApiConfigRaw with a valid model.
    /// If ~/.config/ragit/api.json exists, values from there will override the defaults.
    /// If the default model doesn't exist in the loaded models,
    /// it selects the lowest-cost model instead.
    fn get_default_api_config_raw(&self) -> Result<ApiConfigRaw, Error> {
        // Start with default config
        let mut config = ApiConfigRaw::default();
        
        // Try to load config from home directory
        if let Ok(Some(home_config)) = self.load_api_config_from_home() {
            // Override default values with values from home config
            if home_config.api_key.is_some() {
                config.api_key = home_config.api_key;
            }
            
            if !home_config.model.is_empty() {
                config.model = home_config.model;
            }
            
            if home_config.timeout.is_some() {
                config.timeout = home_config.timeout;
            }
            
            config.sleep_between_retries = home_config.sleep_between_retries;
            config.max_retry = home_config.max_retry;
            
            if home_config.sleep_after_llm_call.is_some() {
                config.sleep_after_llm_call = home_config.sleep_after_llm_call;
            }
            
            config.dump_log = home_config.dump_log;
            config.dump_api_usage = home_config.dump_api_usage;
        }
        
        // Check if the model exists in the loaded models
        let model_exists = ragit_api::get_model_by_name(&self.models, &config.model).is_ok();
        
        if !model_exists && !self.models.is_empty() {
            // Find the lowest-cost model
            if let Some(lowest_cost_model) = self.find_lowest_cost_model() {
                // Update the model in the config
                config.model = lowest_cost_model.name.clone();
                eprintln!("Warning: Model '{}' not found in models.json. Using lowest-cost model '{}' instead.", 
                         config.model, lowest_cost_model.name);
            }
        }
        
        Ok(config)
    }
}
