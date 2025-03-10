use crate::methods::*;
use crate::utils::fetch_form_data;
use ragit_fs::{
    exists,
    initialize_log_file,
    remove_dir_all,
    set_log_file_path,
    write_log,
};
use std::collections::HashMap;
use warp::Filter;
use warp::filters::multipart::FormData;
use warp::http::status::StatusCode;
use warp::reply::with_status;

mod error;
mod methods;
mod models;
mod utils;

#[tokio::main]
async fn main() {
    initinalize_server();
    write_log("server", "hello from ragit-server!");

    let get_index_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("index"))
        .map(get_index);

    let get_config_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("config"))
        .and(warp::path::param::<String>())
        .map(get_config);

    let get_prompt_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("prompt"))
        .and(warp::path::param::<String>())
        .map(get_prompt);

    let get_chunk_count_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chunk-count"))
        .map(get_chunk_count);

    let get_chunk_list_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chunk-list"))
        .and(warp::path::param::<String>())
        .map(get_chunk_list);

    let get_chunk_list_all_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chunk-list"))
        .and(warp::path::end())
        .map(get_chunk_list_all);

    let get_chunk_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chunk"))
        .and(warp::path::param::<String>())
        .map(get_chunk);

    let get_image_list_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("image-list"))
        .and(warp::path::param::<String>())
        .map(get_image_list);

    let get_image_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("image"))
        .and(warp::path::param::<String>())
        .map(get_image);

    let get_image_desc_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("image-desc"))
        .and(warp::path::param::<String>())
        .map(get_image_desc);

    let get_cat_file_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("cat-file"))
        .and(warp::path::param::<String>())
        .map(get_cat_file);

    let get_archive_list_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("archive-list"))
        .map(get_archive_list);

    let get_archive_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("archive"))
        .and(warp::path::param::<String>())
        .map(get_archive);

    let get_meta_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("meta"))
        .map(get_meta);

    let get_version_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("version"))
        .map(get_version);

    let get_server_version_handler = warp::get()
        .and(warp::path("version"))
        .map(get_server_version);

    let get_user_list_handler = warp::get()
        .and(warp::path("user-list"))
        .and(warp::path::end())
        .map(get_user_list);

    let get_repo_list_handler = warp::get()
        .and(warp::path("repo-list"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .map(get_repo_list);

    let get_chat_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chat"))
        .and(warp::path::param::<String>())
        .map(get_chat);

    let get_chat_list_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chat-list"))
        .and(warp::query::<HashMap<String, String>>())
        .map(get_chat_list);

    let get_file_list_handler = warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("file-list"))
        .map(get_file_list);

    let post_begin_push_handler = warp::post()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("begin-push"))
        .and(warp::header::optional::<String>("authorization"))
        .map(post_begin_push);

    let post_archive_handler = warp::post()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("archive"))
        .and(warp::path::end())
        .and(warp::multipart::form())
        .then(post_archive);

    let post_finalize_push_handler = warp::post()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("finalize-push"))
        .and(warp::path::end())
        .and(warp::body::bytes())
        .map(post_finalize_push);

    let create_chat_handler = warp::post()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chat-list"))
        .and(warp::path::end())
        .map(create_chat);

    // NOTE: warp::body::form::<HashMap<String, String>> can catch `application/x-www-form-urlencoded`,
    //       but `warp::body::form::<HashMap<String, Vec<u8>>>` cannot. Is this an upstream issue?
    let post_chat_handler_body_form = warp::post()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chat"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::body::form::<HashMap<String, String>>())
        .then(async |user: String, repo: String, chat_id: String, form: HashMap<String, String>| {
            post_chat(
                user,
                repo,
                chat_id,
                form.into_iter().map(|(key, value)| (key, value.as_bytes().to_vec())).collect(),
            ).await
        });

    // TODO: what's the difference between multipart::form and body::form? I'm noob to this...
    let post_chat_handler_multipart_form = warp::post()
        .and(warp::path::param::<String>())
        .and(warp::path::param::<String>())
        .and(warp::path("chat"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::multipart::form())
        .then(async |user: String, repo: String, chat_id: String, form: FormData| {
            match fetch_form_data(form).await.handle_error(400) {
                Ok(form) => post_chat(user, repo, chat_id, form).await,
                Err((code, _)) => Box::new(with_status(String::new(), StatusCode::from_u16(code).unwrap())),
            }
        });

    let not_found_handler = warp::get().map(not_found);

    warp::serve(
        get_server_version_handler
            .or(get_user_list_handler)
            .or(get_repo_list_handler)
            .or(get_index_handler)
            .or(get_config_handler)
            .or(get_prompt_handler)
            .or(get_chunk_count_handler)
            .or(get_chunk_list_handler)
            .or(get_chunk_list_all_handler)
            .or(get_chunk_handler)
            .or(get_image_list_handler)
            .or(get_image_handler)
            .or(get_image_desc_handler)
            .or(get_cat_file_handler)
            .or(get_archive_list_handler)
            .or(get_archive_handler)
            .or(get_meta_handler)
            .or(get_version_handler)
            .or(get_chat_handler)
            .or(get_chat_list_handler)
            .or(get_file_list_handler)
            .or(post_begin_push_handler)
            .or(post_archive_handler)
            .or(post_finalize_push_handler)
            .or(post_chat_handler_body_form)
            .or(post_chat_handler_multipart_form)
            .or(create_chat_handler)
            .or(not_found_handler)
            .with(warp::log::custom(
                |info| {
                    let headers = info.request_headers();

                    write_log(
                        &info.remote_addr().map(
                            |remote_addr| remote_addr.to_string()
                        ).unwrap_or_else(|| String::from("NO REMOTE ADDR")),
                        &format!(
                            "{:4} {:16} {:4} {headers:?}",
                            info.method().as_str(),
                            info.path(),
                            info.status().as_u16(),
                        ),
                    );
                }
            ))
    ).run(([0, 0, 0, 0], 41127)).await;  // TODO: configurable port number
}

fn initinalize_server() {
    set_log_file_path(Some("ragit-server-logs".to_string()));
    initialize_log_file("ragit-server-logs", true).unwrap();

    if exists("./session") {
        remove_dir_all("./session").unwrap();
    }
}
