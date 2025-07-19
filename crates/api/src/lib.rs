use ragit_fs::{WriteMode, read_string, write_string};

mod api_provider;
pub mod audit;
mod error;
mod message;
mod model;
mod request;
mod response;
mod rate_limit;
pub mod qa_system;

#[cfg(test)]
mod tests;

pub use crate::api_provider::ApiProvider;
pub use crate::audit::AuditRecord;
pub use crate::error::Error;
pub use crate::message::message_contents_to_json_array;
pub use crate::model::{Model, ModelRaw, get_model_by_name, QualityExpectations, TestModel};
pub use crate::request::Request;
pub use crate::response::Response;
pub use crate::qa_system::{ModelQAResult, ModelQASystem, QualityScores};

pub use ragit_pdl::{
    JsonType,
    ImageType,
    Message,
    MessageContent,
    Role,
    Schema,
};

pub fn load_models(json_path: &str) -> Result<Vec<Model>, Error> {
    let models = read_string(json_path)?;
    let models: Vec<ModelRaw> = serde_json::from_str(&models)?;
    let mut result = Vec::with_capacity(models.len());

    for model in models.iter() {
        result.push(Model::try_from(model)?);
    }

    Ok(result)
}

pub fn save_models(models: &[Model], path: &str) -> Result<(), Error> {
    let models: Vec<ModelRaw> = models.iter().map(|model| model.into()).collect();
    Ok(write_string(
        path,
        &serde_json::to_string_pretty(&models)?,
        WriteMode::CreateOrTruncate,
    )?)
}
