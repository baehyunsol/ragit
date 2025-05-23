use super::{HandleError, RawResponse, get_pool, handler};
use crate::{AI_MODEL_CONFIG, CONFIG};
use crate::models::{ai_model, auth};
use crate::models::user::{self, UserCreation};
use ragit_api::JsonType;
use serde_json::Value;
use std::collections::HashMap;
use warp::http::StatusCode;
use warp::reply::{Reply, json, with_status};

pub async fn get_user_list(query: HashMap<String, String>, api_key: Option<String>) -> Box<dyn Reply> {
    handler(get_user_list_(query, api_key).await)
}

async fn get_user_list_(query: HashMap<String, String>, api_key: Option<String>) -> RawResponse {
    let pool = get_pool().await;
    let limit = query.get("limit").map(|s| s.as_ref()).unwrap_or("50").parse::<i64>().unwrap_or(50);
    let offset = query.get("offset").map(|s| s.as_ref()).unwrap_or("0").parse::<i64>().unwrap_or(0);
    let include_privates = auth::is_admin(api_key, pool).await.handle_error(500)?;
    let users = user::get_list(include_privates, limit, offset, pool).await.handle_error(500)?;

    // TODO: if a private user X requests this api with his api key, it should include X but doesn't

    Ok(Box::new(json(&users)))
}

pub async fn get_user(user: String, api_key: Option<String>) -> Box<dyn Reply> {
    handler(get_user_(user, api_key).await)
}

async fn get_user_(user: String, api_key: Option<String>) -> RawResponse {
    let pool = get_pool().await;
    let user = user::get_detail(&user, pool).await.handle_error(404)?;

    if !user.public {
        user::check_auth(&user.id, api_key, pool).await.handle_error(500)?.handle_error(404)?;
    }

    Ok(Box::new(json(&user)))
}

pub async fn create_user(body: Value, api_key: Option<String>) -> Box<dyn Reply> {
    handler(create_user_(body, api_key).await)
}

async fn create_user_(body: Value, api_key: Option<String>) -> RawResponse {
    let pool = get_pool().await;
    let config = CONFIG.get().handle_error(500)?;
    let no_user_at_all = user::no_user_at_all(pool).await.handle_error(500)?;

    if !no_user_at_all && config.only_admin_can_create_user {
        auth::is_admin(api_key, pool).await.handle_error(500)?.handle_error(403)?;
    }

    let user = serde_json::from_value::<UserCreation>(body).handle_error(400)?;
    user::create(&user, pool).await.handle_error(500)?;
    let ai_model_config = AI_MODEL_CONFIG.get().handle_error(500)?;

    for model in ai_model_config.default_models.iter() {
        let model_id = ai_model::create_and_return_id(model, pool).await.handle_error(500)?;
        ai_model::register(
            &user.id,
            &model_id,
            None,  // api_key
            model.name == ai_model_config.default_model,  // default model
            pool,
        ).await.handle_error(500)?;
    }

    Ok(Box::new(with_status(String::new(), StatusCode::from_u16(200).unwrap())))
}

pub async fn get_ai_model_list(user: String, api_key: Option<String>) -> Box<dyn Reply> {
    handler(get_ai_model_list_(user, api_key).await)
}

async fn get_ai_model_list_(user: String, api_key: Option<String>) -> RawResponse {
    let pool = get_pool().await;

    // TODO: do I have to allow everyone to see the model list of a public user?
    user::check_auth(&user, api_key, pool).await.handle_error(500)?.handle_error(404)?;

    let model_list = ai_model::get_list_by_user_id(&user, pool).await.handle_error(500)?;
    Ok(Box::new(json(&model_list)))
}

pub async fn put_ai_model_list(user: String, form: Value, api_key: Option<String>) -> Box<dyn Reply> {
    handler(put_ai_model_list_(user, form, api_key).await)
}

async fn put_ai_model_list_(user: String, form: Value, api_key: Option<String>) -> RawResponse {
    let pool = get_pool().await;
    user::check_auth(&user, api_key, pool).await.handle_error(500)?.handle_error(404)?;

    let Value::Object(form) = form else {
        return Err((400, format!("Expected a json object, got `{:?}`", JsonType::from(&form))));
    };

    match form.get("default_model") {
        Some(Value::String(default_model)) => {
            ai_model::set_default_model(&user, default_model, pool).await.handle_error(404)?;
            return Ok(Box::new(with_status(String::new(), StatusCode::from_u16(200).unwrap())));
        },
        Some(v) => {
            return Err((400, format!("Expected a string, got `{:?}`", JsonType::from(v))));
        },
        None => {},
    }

    let model_name = match form.get("model") {
        Some(Value::String(name)) => name,
        Some(v) => {
            return Err((400, format!("Expected a string, got `{:?}`", JsonType::from(v))));
        },
        None => {
            return Err((400, format!("Key `model` is missing")));
        },
    };

    if let Some(Value::String(api_key)) = form.get("api_key") {
        // If it fails, that's likely because `model_name` is wrong
        ai_model::update_api_key(&user, model_name, Some(api_key.to_string()), pool).await.handle_error(404)?;
    }

    else if let Some(Value::Null) = form.get("api_key") {
        ai_model::update_api_key(&user, model_name, None, pool).await.handle_error(404)?;
    }

    else if let Some(v) = form.get("api_key") {
        return Err((400, format!("Expected a string or null, got `{:?}`", JsonType::from(v))));
    }

    // TODO: handle more fields

    Ok(Box::new(with_status(
        String::new(),
        StatusCode::from_u16(200).unwrap(),
    )))
}
