use crate::api_provider::ApiProvider;
use crate::error::Error;
use ragit_pdl::Message;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write, stdin, stdout};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Model {
    pub name: String,
    pub api_name: String,
    pub can_read_images: bool,
    pub api_provider: ApiProvider,
    pub dollars_per_1b_input_tokens: u64,
    pub dollars_per_1b_output_tokens: u64,
    pub api_timeout: u64,
    pub explanation: Option<String>,
    pub api_key: Option<String>,
    pub api_env_var: Option<String>,
}

impl Model {
    pub fn dummy() -> Self {
        Model {
            name: String::from("dummy"),
            api_name: String::new(),
            can_read_images: false,
            api_provider: ApiProvider::Test(TestModel::Dummy),
            dollars_per_1b_input_tokens: 0,
            dollars_per_1b_output_tokens: 0,
            api_timeout: 180,
            explanation: None,
            api_key: None,
            api_env_var: None,
        }
    }

    pub fn stdin() -> Self {
        Model {
            name: String::from("stdin"),
            api_name: String::new(),
            can_read_images: false,
            api_provider: ApiProvider::Test(TestModel::Stdin),
            dollars_per_1b_input_tokens: 0,
            dollars_per_1b_output_tokens: 0,
            api_timeout: 180,
            explanation: None,
            api_key: None,
            api_env_var: None,
        }
    }

    pub fn get_api_url(&self) -> &str {
        self.api_provider.get_api_url()
    }

    pub fn get_api_key(&self) -> Result<String, Error> {
        // First, check if the API key is directly set in the model
        if let Some(key) = &self.api_key {
            return Ok(key.to_string());
        }
        
        // Next, check if an environment variable is specified and try to get the API key from it
        if let Some(var) = &self.api_env_var {
            if let Ok(key) = std::env::var(var) {
                return Ok(key.to_string());
            }
            
            // Don't return an error yet, try the other methods first
        }
        
        // If we get here, try to find the API key in external model files
        if let Some(key) = self.find_api_key_in_external_files()? {
            return Ok(key);
        }
        
        // If we have an api_env_var but couldn't find the key anywhere, return an error
        if let Some(var) = &self.api_env_var {
            return Err(Error::ApiKeyNotFound { env_var: Some(var.to_string()) });
        }
        
        // If both `api_key` and `api_env_var` are not set,
        // it assumes that the model does not require an
        // api key.
        Ok(String::new())
    }
    
    fn find_api_key_in_external_files(&self) -> Result<Option<String>, Error> {
        // Try to find the API key in the file indicated by RAGIT_MODEL_FILE
        if let Ok(file_path) = std::env::var("RAGIT_MODEL_FILE") {
            if let Some(key) = self.find_api_key_in_file(&file_path)? {
                return Ok(Some(key));
            }
        }
        
        // Try to find the API key in ~/.config/ragit/models.json
        if let Ok(home_dir) = std::env::var("HOME") {
            let config_path = format!("{}/.config/ragit/models.json", home_dir);
            if let Some(key) = self.find_api_key_in_file(&config_path)? {
                return Ok(Some(key));
            }
        }
        
        Ok(None)
    }
    
    fn find_api_key_in_file(&self, file_path: &str) -> Result<Option<String>, Error> {
        use std::fs::File;
        use std::io::Read;
        
        // Check if the file exists
        let file = match File::open(file_path) {
            Ok(file) => file,
            Err(_) => return Ok(None), // File doesn't exist or can't be opened
        };
        
        // Read the file content
        let mut content = String::new();
        if let Err(_) = file.take(10_000_000).read_to_string(&mut content) {
            return Ok(None); // Can't read the file
        }
        
        // Parse the JSON
        let models: Vec<ModelRaw> = match serde_json::from_str(&content) {
            Ok(models) => models,
            Err(_) => return Ok(None), // Can't parse the JSON
        };
        
        // Find the model with the same name
        for model in models {
            if model.name == self.name {
                // If the model has an API key, return it
                if let Some(key) = model.api_key {
                    return Ok(Some(key));
                }
                
                // If the model has an environment variable, try to get the API key from it
                if let Some(var) = model.api_env_var {
                    if let Ok(key) = std::env::var(&var) {
                        return Ok(Some(key));
                    }
                }
            }
        }
        
        Ok(None)
    }

    pub fn default_models() -> Vec<Model> {
        ModelRaw::default_models().iter().map(
            |model| model.try_into().unwrap()
        ).collect()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelRaw {
    /// Model name shown to user.
    /// `rag config --set model` also
    /// uses this name.
    name: String,

    /// Model name used for api requests.
    api_name: String,

    can_read_images: bool,

    /// `openai | cohere | anthropic`
    ///
    /// If you're using an openai-compatible
    /// api, set this to `openai`.
    api_provider: String,

    /// It's necessary if you're using an
    /// openai-compatible api. If it's not
    /// set, ragit uses the default url of
    /// each api provider.
    api_url: Option<String>,

    /// Dollars per 1 million input tokens.
    input_price: f64,

    /// Dollars per 1 million output tokens.
    output_price: f64,

    /// The number is in seconds.
    /// If not set, it's default to 180 seconds.
    api_timeout: Option<u64>,

    explanation: Option<String>,

    /// If you don't want to use an env var, you
    /// can hard-code your api key in this field.
    api_key: Option<String>,

    /// If you've hard-coded your api key,
    /// you don't have to set this. If neither
    /// `api_key`, nor `api_env_var` is set,
    /// it assumes that the model doesn't require
    /// an api key.
    api_env_var: Option<String>,
}

impl ModelRaw {
    pub(crate) fn llama_70b() -> Self {
        ModelRaw {
            name: String::from("llama3.3-70b-groq"),
            api_name: String::from("llama-3.3-70b-versatile"),
            can_read_images: false,
            api_provider: String::from("openai"),
            api_url: Some(String::from("https://api.groq.com/openai/v1/chat/completions")),
            input_price: 0.59,
            output_price: 0.79,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("GROQ_API_KEY")),
        }
    }

    pub(crate) fn llama_8b() -> Self {
        ModelRaw {
            name: String::from("llama3.1-8b-groq"),
            api_name: String::from("llama-3.1-8b-instant"),
            can_read_images: false,
            api_provider: String::from("openai"),
            api_url: Some(String::from("https://api.groq.com/openai/v1/chat/completions")),
            input_price: 0.05,
            output_price: 0.08,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("GROQ_API_KEY")),
        }
    }

    pub(crate) fn gpt_4o() -> Self {
        ModelRaw {
            name: String::from("gpt-4o"),
            api_name: String::from("gpt-4o"),
            can_read_images: true,
            api_provider: String::from("openai"),
            api_url: Some(String::from("https://api.openai.com/v1/chat/completions")),
            input_price: 2.5,
            output_price: 10.0,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("OPENAI_API_KEY")),
        }
    }

    pub(crate) fn gpt_4o_mini() -> Self {
        ModelRaw {
            name: String::from("gpt-4o-mini"),
            api_name: String::from("gpt-4o-mini"),
            can_read_images: true,
            api_provider: String::from("openai"),
            api_url: Some(String::from("https://api.openai.com/v1/chat/completions")),
            input_price: 0.15,
            output_price: 0.6,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("OPENAI_API_KEY")),
        }
    }

    pub(crate) fn sonnet() -> Self {
        ModelRaw {
            name: String::from("claude-3.5-sonnet"),
            api_name: String::from("claude-3-5-sonnet-20240620"),
            can_read_images: true,
            api_provider: String::from("anthropic"),
            api_url: Some(String::from("https://api.anthropic.com/v1/messages")),
            input_price: 3.0,
            output_price: 15.0,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("ANTHROPIC_API_KEY")),
        }
    }

    pub(crate) fn phi_4_14b() -> Self {
        ModelRaw {
            name: String::from("phi-4-14b-ollama"),
            api_name: String::from("phi4:14b"),
            can_read_images: true,
            api_provider: String::from("openai"),
            api_url: Some(String::from("http://127.0.0.1:11434/v1/chat/completions")),
            input_price: 0.0,
            output_price: 0.0,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: None,
        }
    }

    pub(crate) fn command_r() -> Self {
        ModelRaw {
            name: String::from("command-r"),
            api_name: String::from("command-r"),
            can_read_images: true,
            api_provider: String::from("cohere"),
            api_url: Some(String::from("https://api.cohere.com/v2/chat")),
            input_price: 0.15,
            output_price: 0.6,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("COHERE_API_KEY")),
        }
    }

    pub(crate) fn command_r_plus() -> Self {
        ModelRaw {
            name: String::from("command-r-plus"),
            api_name: String::from("command-r-plus"),
            can_read_images: true,
            api_provider: String::from("cohere"),
            api_url: Some(String::from("https://api.cohere.com/v2/chat")),
            input_price: 2.5,
            output_price: 10.0,
            api_timeout: None,
            explanation: None,
            api_key: None,
            api_env_var: Some(String::from("COHERE_API_KEY")),
        }
    }

    pub fn default_models() -> Vec<ModelRaw> {
        vec![
            ModelRaw::llama_70b(),
            ModelRaw::llama_8b(),
            ModelRaw::gpt_4o(),
            ModelRaw::gpt_4o_mini(),
            ModelRaw::sonnet(),
            ModelRaw::command_r(),
            ModelRaw::command_r_plus(),
            ModelRaw::phi_4_14b(),
        ]
    }
}

pub fn get_model_by_name(models: &[Model], name: &str) -> Result<Model, Error> {
    let mut partial_matches = vec![];

    for model in models.iter() {
        if model.name == name {
            return Ok(model.clone());
        }

        if partial_match(&model.name, name) {
            partial_matches.push(model);
        }
    }

    if partial_matches.len() == 1 {
        Ok(partial_matches[0].clone())
    }

    else if name == "dummy" {
        Ok(Model::dummy())
    }

    else if name == "stdin" {
        Ok(Model::stdin())
    }

    else{
        Err(Error::InvalidModelName {
            name: name.to_string(),
            candidates: partial_matches.iter().map(
                |model| model.name.to_string()
            ).collect(),
        })
    }
}

impl TryFrom<&ModelRaw> for Model {
    type Error = Error;

    fn try_from(m: &ModelRaw) -> Result<Model, Error> {
        Ok(Model {
            name: m.name.clone(),
            api_name: m.api_name.clone(),
            can_read_images: m.can_read_images,
            api_provider: ApiProvider::parse(
                &m.api_provider,
                &m.api_url,
            )?,
            dollars_per_1b_input_tokens: (m.input_price * 1000.0).round() as u64,
            dollars_per_1b_output_tokens: (m.output_price * 1000.0).round() as u64,
            api_timeout: m.api_timeout.unwrap_or(180),
            explanation: m.explanation.clone(),
            api_key: m.api_key.clone(),
            api_env_var: m.api_env_var.clone(),
        })
    }
}

impl From<&Model> for ModelRaw {
    fn from(m: &Model) -> ModelRaw {
        ModelRaw {
            name: m.name.clone(),
            api_name: m.api_name.clone(),
            can_read_images: m.can_read_images,
            api_provider: m.api_provider.to_string(),
            api_url: Some(m.get_api_url().to_string()),
            input_price: m.dollars_per_1b_input_tokens as f64 / 1000.0,
            output_price: m.dollars_per_1b_output_tokens as f64 / 1000.0,
            api_timeout: Some(m.api_timeout),
            explanation: m.explanation.clone(),
            api_key: m.api_key.clone(),
            api_env_var: m.api_env_var.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TestModel {
    Dummy,  // it always returns `"dummy"`
    Stdin,
}

impl TestModel {
    pub fn get_dummy_response(&self, messages: &[Message]) -> String {
        match self {
            TestModel::Dummy => String::from("dummy"),
            TestModel::Stdin => {
                for message in messages.iter() {
                    println!(
                        "<|{:?}|>\n\n{}\n\n",
                        message.role,
                        message.content.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(""),
                    );
                }

                print!("<|Assistant|>\n\n>>> ");
                stdout().flush().unwrap();

                let mut s = String::new();
                stdin().read_to_string(&mut s).unwrap();
                s
            },
        }
    }
}

fn partial_match(haystack: &str, needle: &str) -> bool {
    let h_bytes = haystack.bytes().collect::<Vec<_>>();
    let n_bytes = needle.bytes().collect::<Vec<_>>();
    let mut h_cursor = 0;
    let mut n_cursor = 0;

    while h_cursor < h_bytes.len() && n_cursor < n_bytes.len() {
        if h_bytes[h_cursor] == n_bytes[n_cursor] {
            h_cursor += 1;
            n_cursor += 1;
        }

        else {
            h_cursor += 1;
        }
    }

    n_cursor == n_bytes.len()
}
