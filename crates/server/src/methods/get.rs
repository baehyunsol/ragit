use super::{HandleError, RawResponse, handler};
use crate::models::Chat;
use crate::utils::get_rag_path;
use ragit::{
    Index,
    LoadMode,
    UidQueryConfig,
    chunk,
    merge_and_convert_chunks,
};
use ragit_fs::{
    basename,
    exists,
    extension,
    file_name,
    is_dir,
    join,
    join3,
    join4,
    join5,
    read_bytes,
    read_dir,
    read_string,
    set_extension,
};
use serde_json::Value;
use std::collections::HashMap;
use warp::Reply;
use warp::reply::{json, with_header};

pub fn get_index(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_index_(user, repo))
}

fn get_index_(user: String, repo: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let index_path = join(&rag_path, "index.json").handle_error(400)?;
    let j = read_string(&index_path).handle_error(404)?;

    Ok(Box::new(with_header(
        j,
        "Content-Type",
        "application/json",
    )))
}

pub fn get_config(user: String, repo: String, config: String) -> Box<dyn Reply> {
    handler(get_config_(user, repo, config))
}

fn get_config_(user: String, repo: String, config: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let config_path = join3(
        &rag_path,
        "configs",
        &set_extension(&config, "json").handle_error(400)?,
    ).handle_error(404)?;
    let j = read_string(&config_path).handle_error(404)?;

    Ok(Box::new(with_header(
        j,
        "Content-Type",
        "application/json",
    )))
}

pub fn get_prompt(user: String, repo: String, prompt: String) -> Box<dyn Reply> {
    handler(get_prompt_(user, repo, prompt))
}

fn get_prompt_(user: String, repo: String, prompt: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let prompt_path = join3(
        &rag_path,
        "prompts",
        &set_extension(&prompt, "pdl").handle_error(400)?,
    ).handle_error(400)?;
    let p = read_string(&prompt_path).handle_error(404)?;

    Ok(Box::new(with_header(
        p,
        "Content-Type",
        "text/plain; charset=utf-8",
    )))
}

pub fn get_chunk_count(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_chunk_count_(user, repo))
}

fn get_chunk_count_(user: String, repo: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let index_path = join(&rag_path, "index.json").handle_error(400)?;
    let index_json = read_string(&index_path).handle_error(404)?;
    let index = serde_json::from_str::<Value>(&index_json).handle_error(500)?;

    let (code, error) = match index {
        Value::Object(obj) => match obj.get("chunk_count") {
            Some(Value::Number(n)) => match n.as_u64() {
                Some(n) => { return Ok(Box::new(json(&n))); },
                _ => (500, format!("`{n:?}` is not a valid chunk_count")),
            },
            Some(x) => (500, format!("`{x:?}` is not a valid integer")),
            None => (500, format!("`{index_path}` has no field `chunk_count`")),
        },
        index => (500, format!("`{index:?}` is not a valid index")),
    };

    Err((code, error))
}

pub fn get_chunk_list(user: String, repo: String, prefix: String) -> Box<dyn Reply> {
    handler(get_chunk_list_(user, repo, prefix))
}

fn get_chunk_list_(user: String, repo: String, prefix: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let chunk_path = join3(
        &rag_path,
        "chunks",
        &prefix,
    ).handle_error(400)?;
    let chunks = read_dir(&chunk_path, false).unwrap_or(vec![]);
    Ok(Box::new(json(
        &chunks.iter().filter_map(
            |chunk| match extension(chunk) {
                Ok(Some(e)) if e == "chunk" => file_name(chunk).ok().map(|suffix| format!("{prefix}{suffix}")),
                _ => None,
            }
        ).collect::<Vec<String>>(),
    )))
}

pub fn get_chunk_list_all(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_chunk_list_all_(user, repo))
}

fn get_chunk_list_all_(user: String, repo: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let chunk_parents = join(
        &rag_path,
        "chunks",
    ).handle_error(400)?;
    let mut result = vec![];

    for prefix in 0..256 {
        let prefix = format!("{prefix:02x}");
        let chunks_at = join(
            &chunk_parents,
            &prefix,
        ).handle_error(400)?;

        if exists(&chunks_at) {
            for chunk in read_dir(&chunks_at, false).unwrap_or(vec![]) {
                if extension(&chunk).unwrap_or(None).unwrap_or(String::new()) == "chunk" {
                    result.push(format!("{prefix}{}", file_name(&chunk).handle_error(500)?));
                }
            }
        }
    }

    Ok(Box::new(json(&result)))
}

pub fn get_chunk(user: String, repo: String, uid: String) -> Box<dyn Reply> {
    handler(get_chunk_(user, repo, uid))
}

fn get_chunk_(user: String, repo: String, uid: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(404)?;
    let prefix = uid.get(0..2).ok_or_else(|| format!("invalid uid: {uid}")).handle_error(400)?.to_string();
    let suffix = uid.get(2..).ok_or_else(|| format!("invalid uid: {uid}")).handle_error(400)?.to_string();
    let chunk_path = join4(
        &rag_path,
        "chunks",
        &prefix,
        &set_extension(&suffix, "chunk").handle_error(400)?,
    ).handle_error(400)?;
    let chunk = chunk::load_from_file(&chunk_path).handle_error(404)?;
    let json_str = serde_json::to_string(&chunk).handle_error(500)?;

    Ok(Box::new(with_header(
        // '\n' is for backward-compatibility with older versions
        format!("\n{json_str}"),
        "Content-Type",
        "application/json",
    )))
}

pub fn get_image_list(user: String, repo: String, prefix: String) -> Box<dyn Reply> {
    handler(get_image_list_(user, repo, prefix))
}

fn get_image_list_(user: String, repo: String, prefix: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let image_path = join3(
        &rag_path,
        "images",
        &prefix,
    ).handle_error(400)?;
    let images = read_dir(&image_path, false).handle_error(404)?;

    Ok(Box::new(json(
        &images.iter().filter_map(
            |image| match extension(image) {
                Ok(Some(png)) if png == "png" => file_name(image).ok().map(|suffix| format!("{prefix}{suffix}")),
                _ => None,
            }
        ).collect::<Vec<String>>(),
    )))
}

pub fn get_image(user: String, repo: String, uid: String) -> Box<dyn Reply> {
    handler(get_image_(user, repo, uid))
}

fn get_image_(user: String, repo: String, uid: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let prefix = uid.get(0..2).ok_or_else(|| format!("invalid uid: {uid}")).handle_error(400)?.to_string();
    let suffix = uid.get(2..).ok_or_else(|| format!("invalid uid: {uid}")).handle_error(400)?.to_string();
    let image_path = join4(
        &rag_path,
        "images",
        &prefix,
        &set_extension(&suffix, "png").handle_error(400)?,
    ).handle_error(400)?;
    let bytes = read_bytes(&image_path).handle_error(404)?;

    Ok(Box::new(with_header(
        bytes,
        "Content-Type",
        "image/png",
    )))
}

pub fn get_image_desc(user: String, repo: String, uid: String) -> Box<dyn Reply> {
    handler(get_image_desc_(user, repo, uid))
}

fn get_image_desc_(user: String, repo: String, uid: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(404)?;
    let prefix = uid.get(0..2).ok_or_else(|| format!("invalid uid: {uid}")).handle_error(400)?.to_string();
    let suffix = uid.get(2..).ok_or_else(|| format!("invalid uid: {uid}")).handle_error(400)?.to_string();
    let image_path = join4(
        &rag_path,
        "images",
        &prefix,
        &set_extension(&suffix, "json").handle_error(400)?,
    ).handle_error(400)?;
    let bytes = read_bytes(&image_path).handle_error(404)?;

    Ok(Box::new(with_header(
        bytes,
        "Content-Type",
        "application/json",
    )))
}

pub fn get_cat_file(user: String, repo: String, uid: String) -> Box<dyn Reply> {
    handler(get_cat_file_(user, repo, uid))
}

fn get_cat_file_(user: String, repo: String, uid: String) -> RawResponse {
    let rag_path = join3(
        "data",
        &user,
        &repo,
    ).handle_error(400)?;
    let index = Index::load(rag_path, LoadMode::OnlyJson).handle_error(404)?;
    let query = index.uid_query(&[uid.clone()], UidQueryConfig::new()).handle_error(400)?;

    if query.has_multiple_matches() {
        Err((400, format!("There are multiple file/chunk that match `{uid}`.")))
    }

    else if let Some(uid) = query.get_chunk_uid() {
        let chunk = index.get_chunk_by_uid(uid).handle_error(500)?;

        Ok(Box::new(with_header(
            chunk.data,
            "Content-Type",
            "text/plain; charset=utf-8",
        )))
    }

    else if let Some((_, uid)) = query.get_processed_file() {
        let chunk_uids = index.get_chunks_of_file(uid).handle_error(500)?;
        let mut chunks = Vec::with_capacity(chunk_uids.len());

        for chunk_uid in chunk_uids {
            chunks.push(index.get_chunk_by_uid(chunk_uid).handle_error(500)?);
        }

        chunks.sort_by_key(|chunk| chunk.source.sortable_string());
        let chunks = merge_and_convert_chunks(&index, chunks, true /* render_image */).handle_error(500)?;

        let result = match chunks.len() {
            0 => String::new(),
            1 => chunks[0].data.clone(),
            _ => {
                return Err((500, format!("`index.get_chunks_of_file({uid})` returned chunks from different files.")));
            },
        };

        Ok(Box::new(with_header(
            result,
            "Content-Type",
            "text/plain; charset=utf-8",
        )))
    }

    else {
        Err((404, format!("There's no file/chunk that matches `{uid}`")))
    }
}

pub fn get_archive_list(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_archive_list_(user, repo))
}

fn get_archive_list_(user: String, repo: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;

    if !exists(&rag_path) {
        return Err((404, format!("No such repo: `{user}/{repo}`")));
    }

    let archive_path = join(&rag_path, "archives").handle_error(404)?;
    let archives: Vec<String> = read_dir(&archive_path, true).unwrap_or(vec![]).iter().map(
        |f| basename(&f).unwrap_or(String::new())
    ).filter(
        |f| !f.is_empty()
    ).collect();
    Ok(Box::new(json(&archives)))
}

pub fn get_archive(user: String, repo: String, archive_key: String) -> Box<dyn Reply> {
    handler(get_archive_(user, repo, archive_key))
}

fn get_archive_(user: String, repo: String, archive_key: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let archive_path = join3(&rag_path, "archives", &archive_key).handle_error(400)?;
    let bytes = read_bytes(&archive_path).handle_error(404)?;

    Ok(Box::new(with_header(
        bytes,
        "Content-Type",
        "application/octet-stream",
    )))
}

pub fn get_meta(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_meta_(user, repo))
}

fn get_meta_(user: String, repo: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;

    if !exists(&rag_path) {
        return Err((404, format!("No such repo: `{user}/{repo}`")));
    }

    let meta_path = join(&rag_path, "meta.json").handle_error(400)?;

    // NOTE: a `.ragit/` may or may not have `meta.json`
    let meta_json = read_string(&meta_path).unwrap_or(String::from("{}"));

    Ok(Box::new(with_header(
        meta_json,
        "Content-Type",
        "application/json",
    )))
}

pub fn get_version(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_version_(user, repo))
}

fn get_version_(user: String, repo: String) -> RawResponse {
    let rag_path = get_rag_path(&user, &repo).handle_error(400)?;
    let index_path = join(&rag_path, "index.json").handle_error(400)?;
    let index_json = read_string(&index_path).handle_error(404)?;
    let index = serde_json::from_str::<Value>(&index_json).handle_error(500)?;

    let (code, error) = match index {
        Value::Object(obj) => match obj.get("ragit_version") {
            Some(v) => match v.as_str() {
                Some(v) => {
                    return Ok(Box::new(with_header(
                        v.to_string(),
                        "Content-Type",
                        "text/plain; charset=utf-8",
                    )));
                },
                None => (500, format!("`{v:?}` is not a valid string")),
            },
            None => (500, format!("`{index_path}` has no `ragit_version` field")),
        },
        index => (500, format!("`{index:?}` is not a valid index")),
    };

    Err((code, error))
}

pub fn get_server_version() -> Box<dyn Reply> {
    Box::new(with_header(
        ragit::VERSION,
        "Content-Type",
        "text/plain; charset=utf-8",
    ))
}

pub fn get_user_list() -> Box<dyn Reply> {
    handler(get_user_list_())
}

fn get_user_list_() -> RawResponse {
    let dir = read_dir("data", true).unwrap_or(vec![]);
    let mut users = vec![];

    for d in dir.iter() {
        if is_dir(d) {
            users.push(file_name(d).handle_error(500)?);
        }
    }

    Ok(Box::new(json(&users)))
}

pub fn get_repo_list(user: String) -> Box<dyn Reply> {
    handler(get_repo_list_(user))
}

fn get_repo_list_(user: String) -> RawResponse {
    let dir = read_dir(&join("data", &user).handle_error(400)?, true).handle_error(404)?;
    let mut repos = vec![];

    for d in dir.iter() {
        if is_dir(d) {
            repos.push(file_name(d).handle_error(500)?);
        }
    }

    Ok(Box::new(json(&repos)))
}

pub fn get_chat(user: String, repo: String, chat_id: String) -> Box<dyn Reply> {
    handler(get_chat_(user, repo, chat_id))
}

fn get_chat_(user: String, repo: String, chat_id: String) -> RawResponse {
    let chat_at = join5(
        "data",
        &user,
        &repo,
        "chats",
        &set_extension(&chat_id, "json").handle_error(400)?,
    ).handle_error(400)?;
    Ok(Box::new(json(&Chat::load_from_file(&chat_at).handle_error(404)?)))
}

pub fn get_chat_list(user: String, repo: String, query: HashMap<String, String>) -> Box<dyn Reply> {
    handler(get_chat_list_(user, repo, query))
}

fn get_chat_list_(user: String, repo: String, query: HashMap<String, String>) -> RawResponse {
    let no_history = query.get("history").map(|s| s.as_ref()).unwrap_or("") == "0";
    let repo_at = join3("data", &user, &repo).handle_error(400)?;

    if !exists(&repo_at) {
        return Err((404, format!("`/{user}/{repo}` not found")));
    }

    let chats_at = join(&repo_at, "chats").handle_error(400)?;

    if !exists(&chats_at) {
        return Ok(Box::new(json::<Vec<Chat>>(&vec![])));
    }

    let mut result = vec![];

    for file in read_dir(&chats_at, true).handle_error(404)? {
        match extension(&file) {
            Ok(Some(e)) if e == "json" => {
                let mut chat = Chat::load_from_file(&file).handle_error(500)?;

                if no_history {
                    chat.history = vec![];
                }

                result.push(chat);
            },
            _ => {},
        }
    }

    Ok(Box::new(json(&result)))
}

pub fn get_file_list(user: String, repo: String) -> Box<dyn Reply> {
    handler(get_file_list_(user, repo))
}

fn get_file_list_(user: String, repo: String) -> RawResponse {
    let rag_path = join3(
        "data",
        &user,
        &repo,
    ).handle_error(400)?;
    let index = Index::load(rag_path, LoadMode::OnlyJson).handle_error(404)?;
    Ok(Box::new(json(&index.processed_files.keys().collect::<Vec<_>>())))
}
