use async_recursion::async_recursion;
use ragit::{
    AddMode,
    AddResult,
    Error,
    Index,
    INDEX_DIR_NAME,
    Keywords,
    LoadMode,
    LsChunk,
    UidQuery,
    merge_and_convert_chunks,
    multi_turn,
    single_turn,
};
use ragit_fs::{
    basename,
    join,
    read_dir,
};
use std::env;
use std::io::Write;

mod cli;
use cli::{
    ArgCount,
    ArgParser,
    ArgType,
};

#[tokio::main]
async fn main() {
    let args = env::args().collect::<Vec<_>>();

    match run(args).await {
        Ok(()) => {},
        Err(e) => {
            // TODO: suggest similar names for some errors
            match e {
                Error::IndexNotFound => {
                    eprintln!("`.ragit/` not found. Make sure that it's a valid ragit repo.");
                },
                Error::InvalidConfigKey(s) => {
                    eprintln!("{s:?} is not a valid key for config.");
                },
                Error::ApiError(g) => match g {
                    ragit_api::Error::InvalidModelKind(m) => {
                        eprintln!(
                            "{m:?} is not a valid name for a chat model. Valid names are\n{}",
                            ragit_api::ChatModel::all_kinds().iter().map(
                                |model| model.to_human_friendly_name()
                            ).collect::<Vec<_>>().join("\n"),
                        );
                    },
                    e => {
                        eprintln!("{e:?}");
                    },
                },
                Error::CliError(e) => {
                    eprintln!("cli error: {e}");
                },
                e => {
                    eprintln!("{e:?}");
                },
            }

            std::process::exit(1);
        },
    }
}

#[async_recursion(?Send)]
async fn run(args: Vec<String>) -> Result<(), Error> {
    let root_dir = find_root().map_err(|_| Error::IndexNotFound);

    match args.get(1).map(|arg| arg.as_str()) {
        Some("add") => {
            let parsed_args = ArgParser::new().flag_with_default(&["--ignore", "--auto", "--force"]).optional_flag(&["--dry-run"]).args(ArgType::Path, ArgCount::Geq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/add.txt"));
                return Ok(());
            }

            let mut index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let add_mode = AddMode::parse_flag(&parsed_args.get_flag(0).unwrap()).unwrap();
            let dry_run = parsed_args.get_flag(1).is_some();

            if dry_run {
                return Err(Error::NotImplemented(String::from("rag add --dry-run")));
            }

            let files = parsed_args.get_args();

            let (mut added, mut updated, mut ignored) = (0, 0, 0);

            for path in files.iter() {
                match index.add_file(path, add_mode)? {
                    AddResult::Added => { added += 1; },
                    AddResult::Updated => { updated += 1; },
                    AddResult::Ignored => { ignored += 1; },
                }
            }

            index.save_to_file()?;
            println!("{added} added files, {updated} updated files, {ignored} ignored files");
        },
        // FIXME: this is a temporary command, only to test the migration function
        //        I have to come up with better policies and cli for migration
        Some("migrate") => {
            Index::migrate(&root_dir?)?;
        },
        Some("build") => {
            let parsed_args = ArgParser::new().parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/build.txt"));
                return Ok(());
            }

            let mut index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            index.build().await?;
        },
        Some("cat-file") => {
            let parsed_args = ArgParser::new().args(ArgType::Query, ArgCount::Exact(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/cat-file.txt"));
                return Ok(());
            }

            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let query = parsed_args.get_args_exact(1)?[0].clone();
            let query_result = index.uid_query(UidQuery::with_query(query.clone()).file_or_chunk())?;

            if query_result.has_multiple_matches() {
                return Err(Error::UidQueryError(format!("There're multiple file/chunk that match `{query}`. Please give more specific query.")));
            }

            else if let Some(uid) = query_result.get_chunk_uid() {
                let chunk = index.get_chunk_by_uid(uid)?;
                println!("{}", chunk.data);
            }

            else if let Some((_, uid)) = query_result.get_processed_file() {
                let chunk_uids = index.get_chunks_of_file(uid)?;
                let mut chunks = Vec::with_capacity(chunk_uids.len());

                for chunk_uid in chunk_uids {
                    chunks.push(index.get_chunk_by_uid(chunk_uid)?);
                }

                chunks.sort_by_key(|chunk| chunk.index);
                let chunks = merge_and_convert_chunks(&index, chunks)?;

                match chunks.len() {
                    0 => {
                        // empty file
                    },
                    1 => {
                        println!("{}", chunks[0].data);
                    },
                    _ => {
                        return Err(Error::BrokenIndex(String::from("Assertion error: `merge_and_convert_chunks` failed to merge chunks of a file. It's likely to be a bug, please open an issue.")));
                    },
                }
            }

            else if let Some(f) = query_result.get_staged_file() {
                return Err(Error::UidQueryError(format!("`{f}` has no chunks yet. Please run `rag build`.")));
            }

            else {
                return Err(Error::UidQueryError(format!("There's no file/chunk that matches `{query}`.")));
            }
        },
        Some("check") => {
            let parsed_args = ArgParser::new().optional_flag(&["--recursive"]).optional_flag(&["--recover"]).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/check.txt"));
                return Ok(());
            }

            let root_dir = root_dir?;
            let recursive = parsed_args.get_flag(0).is_some();
            let recover = parsed_args.get_flag(1).is_some();

            match Index::load(root_dir.clone(), LoadMode::OnlyJson) {
                Ok(mut index) => if index.curr_processing_file.is_some() && recover {
                    let recover_result = index.recover()?;
                    index.save_to_file()?;
                    index.check(recursive)?;
                    println!("recovered from a corrupted knowledge-base: {recover_result:?}");
                } else {
                    match index.check(recursive) {
                        Ok(()) => {
                            println!("everything is fine!");
                        },
                        Err(e) => if recover {
                            let recover_result = index.recover()?;
                            index.save_to_file()?;
                            index.check(recursive)?;
                            println!("recovered from a corrupted knowledge-base: {recover_result:?}");
                        } else {
                            return Err(e);
                        }
                    }
                },
                Err(e) => if recover {
                    let mut index = Index::load(root_dir, LoadMode::Minimum)?;
                    let recover_result = index.recover()?;
                    index.save_to_file()?;
                    index.check(recursive)?;
                    println!("recovered from a corrupted knowledge-base: {recover_result:?}");
                } else {
                    return Err(e);
                },
            }
        },
        Some("clone") => {
            let parsed_args = ArgParser::new().args(ArgType::String, ArgCount::Geq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/clone.txt"));
                return Ok(());
            }

            let args = parsed_args.get_args();
            Index::clone(
                args[0].clone(),
                args.get(1).map(|s| s.to_string()),
            ).await?;
            return Ok(());
        },
        Some("config") => {
            let parsed_args = ArgParser::new().flag(&["--set", "--get", "--get-all"]).args(ArgType::String, ArgCount::Geq(0)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/config.txt"));
                return Ok(());
            }

            let mut index = Index::load(root_dir?, LoadMode::OnlyJson)?;

            match parsed_args.get_flag(0).unwrap().as_str() {
                "--set" => {
                    let args = parsed_args.get_args_exact(2)?;
                    let previous_value = index.set_config_by_key(args[0].clone(), args[1].clone())?;

                    match previous_value {
                        Some(v) => {
                            println!("set `{}`: `{}` -> `{}`", args[0].clone(), v, args[1].clone());
                        },
                        None => {
                            println!("set `{}`: `{}`", args[0].clone(), args[1].clone());
                        },
                    }
                },
                "--get" => {
                    let args = parsed_args.get_args_exact(1)?;
                    println!("{}", index.get_config_by_key(args[0].clone())?.to_string());
                },
                "--get-all" => {
                    parsed_args.get_args_exact(0)?;  // make sure that there's no dangling args
                    let mut kv = index.get_all_configs()?;
                    kv.sort_by_key(|(k, _)| k.to_string());

                    println!("{}", '{');

                    for (k, v) in kv.iter() {
                        println!("    {k:?}: {v},");
                    }

                    println!("{}", '}');
                },
                _ => unreachable!(),
            }
        },
        Some("ext") => {
            let parsed_args = ArgParser::new().args(ArgType::Path, ArgCount::Geq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/ext.txt"));
                return Ok(());
            }

            let mut index = Index::load(root_dir?, LoadMode::OnlyJson)?;
            let bases = parsed_args.get_args();

            for base in bases.iter() {
                index.ext(base)?;
            }

            index.save_to_file()?;
        },
        Some("gc") => {
            let parsed_args = ArgParser::new().flag(&["--logs", "--images"]).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/gc.txt"));
                return Ok(());
            }

            match parsed_args.get_flag(0).unwrap().as_str() {
                "--logs" => {
                    let index = Index::load(root_dir?, LoadMode::OnlyJson)?;
                    let removed = index.gc_logs()?;
                    println!("removed {removed} log files");
                },
                "--images" => {
                    let index = Index::load(root_dir?, LoadMode::QuickCheck)?;
                    let removed = index.gc_images()?;
                    println!("removed {removed} files");
                },
                _ => unreachable!(),
            }
        },
        Some("help") => {
            let parsed_args = ArgParser::new().args(ArgType::Command, ArgCount::Leq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/help.txt"));
                return Ok(());
            }

            match parsed_args.get_args().get(0).map(|arg| arg.as_str()) {
                Some("chunks") => {
                    println!("{}", include_str!("../docs/chunks.md"));
                },
                Some("config-reference") => {
                    println!("{}", include_str!("../docs/config.md"));
                },
                Some("quick-guide") => {
                    println!("{}", include_str!("../docs/quick_guide.md"));
                },
                Some(command) => {
                    let mut new_args = args.clone();
                    new_args[1] = command.to_string();
                    new_args[2] = String::from("--help");
                    return run(new_args).await;
                },
                None => {
                    println!("{}", include_str!("../docs/commands/help.txt"));
                },
            }
        },
        Some("init") => {
            let parsed_args = ArgParser::new().parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/init.txt"));
                return Ok(());
            }

            match Index::new(String::from(".")) {
                Ok(_) => { println!("initialized"); },
                Err(Error::IndexAlreadyExists(_)) => { println!("There already is a knowledge-base here."); },
                Err(e) => { return Err(e); },
            }
        },
        Some("kv") => {
            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let mut result = std::collections::HashMap::new();
            let chunk_uids = index.get_all_chunk_uids()?;

            for uid in chunk_uids.iter() {
                for term in index.get_tfidf_by_chunk_uid(*uid)?.term_frequency.keys() {
                    match result.get_mut(term) {
                        Some(n) => { *n += 1; },
                        None => { result.insert(term.to_string(), 1); },
                    }
                }
            }

            let mut result = result.into_iter().collect::<Vec<_>>();
            result.sort_by_key(|(_, count)| usize::MAX - *count);

            for (term, count) in result.iter() {
                println!("{term}: {count} ({:.3}%)", *count as f64 / chunk_uids.len() as f64 * 100.0);
            }
        },
        Some("ls-chunks") => {
            let parsed_args = ArgParser::new().optional_flag(&["--uid-only", "--stat-only"]).args(ArgType::Query, ArgCount::Leq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/ls-chunks.txt"));
                return Ok(());
            }

            let uid_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--uid-only";
            let stat_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--stat-only";
            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let chunks = match parsed_args.get_args().get(0) {
                Some(arg) => {
                    let query = index.uid_query(UidQuery::with_query(arg.to_string()).file_or_chunk())?;
                    let mut chunks = vec![];

                    for file_uid in query.get_file_uids() {
                        let uids = index.get_chunks_of_file(file_uid)?;

                        for uid in uids.iter() {
                            let chunk = index.get_chunk_by_uid(*uid)?;
                            chunks.push(chunk);
                        }
                    }

                    for uid in query.get_chunk_uids() {
                        let chunk = index.get_chunk_by_uid(uid)?;
                        chunks.push(chunk);
                    }

                    if chunks.is_empty() {
                        return Err(Error::UidQueryError(format!("There's no chunk/file that matches `{arg}`.")));
                    }

                    if !uid_only {
                        println!("{} chunks", chunks.len());
                    }

                    if stat_only {
                        return Ok(());
                    }

                    chunks.into_iter().map(|chunk| LsChunk::from(chunk)).collect()
                },
                None => {
                    if !uid_only {
                        println!("{} chunks", index.chunk_count);
                    }

                    if stat_only {
                        return Ok(());
                    }

                    index.list_chunks(
                        &|_| true,  // no filter
                        &|c| c,  // no map
                        &|chunk: &LsChunk| format!("{}-{:08}", chunk.file, chunk.index),  // sort by file
                    )?
                },
            };

            for chunk in chunks.iter() {
                if uid_only {
                    println!("{}", chunk.uid);
                    continue;
                }

                println!("----------");
                println!("{}th chunk of {}", chunk.index, chunk.file);
                println!("uid: {}", chunk.uid);
                println!("character_len: {}", chunk.character_len);
                println!("title: {}", chunk.title);
                println!("summary: {}", chunk.summary);
            }
        },
        Some("ls-files") => {
            let parsed_args = ArgParser::new().optional_flag(&["--name-only", "--uid-only", "--stat-only"]).args(ArgType::Query, ArgCount::Leq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/ls-files.txt"));
                return Ok(());
            }

            let name_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--name-only";
            let uid_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--uid-only";
            let stat_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--stat-only";
            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let files = match parsed_args.get_args().get(0) {
                Some(arg) => {
                    let query = index.uid_query(UidQuery::with_query(arg.to_string()).file_only())?;
                    let mut files = vec![];
                    let mut processed_files_len = 0;
                    let mut staged_files_len = 0;

                    for (path, uid) in query.get_processed_files() {
                        processed_files_len += 1;
                        files.push(index.get_ls_file(Some(path), Some(uid))?);
                    }

                    for path in query.get_staged_files() {
                        staged_files_len += 1;
                        files.push(index.get_ls_file(Some(path), None)?);
                    }

                    if files.is_empty() {
                        return Err(Error::UidQueryError(format!("There's no file that matches `{arg}`.")));
                    }

                    if !uid_only && !name_only {
                        println!(
                            "{} total files, {} staged files, {} processed files",
                            processed_files_len + staged_files_len,
                            staged_files_len,
                            processed_files_len,
                        );
                    }

                    if stat_only {
                        return Ok(());
                    }

                    files
                },
                None => {
                    if !uid_only && !name_only {
                        println!(
                            "{} total files, {} staged files, {} processed files",
                            index.staged_files.len() + index.processed_files.len() + if index.curr_processing_file.is_some() { 1 } else { 0 },
                            index.staged_files.len(),
                            index.processed_files.len() + if index.curr_processing_file.is_some() { 1 } else { 0 },
                        );
                    }

                    if stat_only {
                        return Ok(());
                    }

                    index.list_files(
                        &|_| true,  // no filter
                        &|f| f,  // no map
                        &|f| f.path.to_string(),
                    )?
                },
            };

            for file in files.iter() {
                if name_only {
                    println!("{}", file.path);
                    continue;
                }

                else if uid_only {
                    println!("{}", file.uid);
                    continue;
                }

                println!("--------");
                println!("name: {}{}", file.path, if file.is_processed { String::new() } else { String::from(" (not processed yet)") });

                if file.is_processed {
                    println!("length: {}", file.length);
                    println!("uid: {}", file.uid);
                    println!("chunks: {}", file.chunks);
                }
            }
        },
        Some("ls-images") => {
            let parsed_args = ArgParser::new().optional_flag(&["--uid-only", "--stat-only"]).args(ArgType::Query, ArgCount::Leq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/ls-images.txt"));
                return Ok(());
            }

            let uid_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--uid-only";
            let stat_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--stat-only";
            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let images = match parsed_args.get_args().get(0) {
                Some(arg) => {
                    let query = index.uid_query(UidQuery::with_query(arg.to_string()))?;
                    let mut image_uids = vec![];

                    for (_, uid) in query.get_processed_files() {
                        for image_uid in index.get_images_of_file(uid)? {
                            image_uids.push(image_uid);
                        }
                    }

                    for uid in query.get_chunk_uids() {
                        let chunk = index.get_chunk_by_uid(uid)?;

                        for image_uid in chunk.images {
                            image_uids.push(image_uid);
                        }
                    }

                    for image_uid in query.get_image_uids() {
                        image_uids.push(image_uid);
                    }

                    if image_uids.is_empty() {
                        return Err(Error::UidQueryError(format!("There's no chunk/file/image that matches `{arg}`.")));
                    }

                    if !uid_only {
                        println!("{} images", image_uids.len());
                    }

                    if stat_only {
                        return Ok(());
                    }

                    let mut result = Vec::with_capacity(image_uids.len());

                    for image_uid in image_uids.iter() {
                        result.push(index.get_ls_image(*image_uid)?);
                    }

                    result
                },
                None => {
                    let result = index.list_images(
                        &|_| true,  // no filter
                        &|image| image,  // no map
                        &|_| 0,  // no sort
                    )?;

                    if !uid_only {
                        println!("{} images", result.len());
                    }

                    if stat_only {
                        return Ok(());
                    }

                    result
                },
            };

            for image in images.iter() {
                if uid_only {
                    println!("{}", image.uid);
                    continue;
                }

                println!("--------");
                println!("uid: {}", image.uid);
                println!("explanation: {}", image.explanation);
                println!("extracted_text: {}", image.extracted_text);
                println!("size: {}", image.size);
            }
        },
        Some("ls-models") => {
            let parsed_args = ArgParser::new().optional_flag(&["--name-only", "--stat-only"]).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/ls-models.txt"));
                return Ok(());
            }

            let name_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--name-only";
            let stat_only = parsed_args.get_flag(0).unwrap_or(String::new()) == "--stat-only";
            let models = Index::list_models(
                &|model| model.name != "dummy",  // filter
                &|model| model,  // no map
                &|model| model.name.to_string(),
            );
            println!("{} models", models.len());

            if stat_only {
                return Ok(());
            }

            for model in models.iter() {
                if name_only {
                    println!("{}", model.name);
                    continue;
                }

                println!("--------");
                println!("name: {}", model.name);
                println!("api_provider: {}", model.api_provider);
                println!("api_key_env_var: {}", model.api_key_env_var.as_ref().map(|k| k.to_string()).unwrap_or(String::new()));
                println!("can_read_images: {}", model.can_read_images);
                println!("dollars_per_1b_input_tokens: {}", model.dollars_per_1b_input_tokens);
                println!("dollars_per_1b_output_tokens: {}", model.dollars_per_1b_output_tokens);
            }
        },
        Some("meta") => {
            let parsed_args = ArgParser::new().flag(&["--get", "--get-all", "--set", "--remove", "--remove-all"]).args(ArgType::String, ArgCount::Geq(0)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/meta.txt"));
                return Ok(());
            }

            let index = Index::load(root_dir?, LoadMode::OnlyJson)?;
            let flag = parsed_args.get_flag(0).unwrap();

            match flag.as_str() {
                "--get" => {
                    let key = &parsed_args.get_args_exact(1)?[0];

                    if let Some(value) = index.get_meta_by_key(key.to_string())? {
                        println!("{value}");
                    }
                },
                "--get-all" => {
                    parsed_args.get_args_exact(0)?;
                    let all = index.get_all_meta()?;
                    println!("{}", json::JsonValue::from(all).pretty(4));
                },
                "--set" => {
                    let key_value = parsed_args.get_args_exact(2)?;

                    index.set_meta_by_key(
                        key_value[0].clone(),
                        key_value[1].clone(),
                    )?;
                    println!("metadata set");  // TODO: show change
                },
                "--remove" => {
                    let key = &parsed_args.get_args_exact(1)?[0];

                    index.remove_meta_by_key(key.to_string())?;
                    println!("removed `{key}`");
                },
                "--remove-all" => {
                    parsed_args.get_args_exact(0)?;
                    index.remove_all_meta()?;
                    println!("metadata removed");
                },
                _ => unreachable!(),
            }
        },
        Some("query") => {
            let parsed_args = ArgParser::new().optional_flag(&["--interactive", "-i"]).args(ArgType::String, ArgCount::Geq(0)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/query.txt"));
                return Ok(());
            }

            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;

            if parsed_args.get_flag(0).is_some() {
                let mut conversation = vec![];

                loop {
                    let mut curr_input = String::new();
                    print!(">>> ");
                    std::io::stdout().flush()?;
                    std::io::stdin().read_line(&mut curr_input)?;
                    conversation.push(curr_input);

                    let result = if conversation.len() == 1 {
                        single_turn(
                            &conversation[0],
                            &index,
                        ).await?
                    } else {
                        multi_turn(
                            conversation.clone(),
                            &index,
                        ).await?
                    };

                    println!("{result}");
                    conversation.push(result);
                }
            }

            else {
                let result = single_turn(
                    &parsed_args.get_args_exact(1)?[0],
                    &index,
                ).await?;
                println!("{result}");
            }
        },
        Some("remove") | Some("rm") => {
            let parsed_args = ArgParser::new().args(ArgType::Path, ArgCount::Geq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/remove.txt"));
                return Ok(());
            }

            let mut index = Index::load(root_dir?, LoadMode::QuickCheck)?;
            let files = parsed_args.get_args();

            let remove_count = if files.len() == 1 && files[0] == "--auto" {
                index.remove_auto()?.len()
            }

            else {
                for file in files.iter() {
                    index.remove_file(file.to_string())?;
                }

                files.len()
            };

            index.save_to_file()?;
            println!("removed {remove_count} files from index");
        },
        Some("reset") => {
            let parsed_args = ArgParser::new().flag(&["--soft", "--hard"]).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/reset.txt"));
                return Ok(());
            }

            let soft = parsed_args.get_flag(0).unwrap() == "--soft";

            if soft {
                let mut index = Index::load(root_dir?, LoadMode::OnlyJson)?;
                index.reset_soft()?;
                index.save_to_file()?;
            }

            else {
                Index::reset_hard(&root_dir?)?;
            }
        },
        // TODO: I would like to introduce `-N=10` flag, which tells at most how many chunks to retrieve.
        //       but the ArgParser doesn't support that kinda arguments
        Some("tfidf") => {
            let parsed_args = ArgParser::new().optional_flag(&["--show"]).args(ArgType::Query, ArgCount::Geq(1)).parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/tfidf.txt"));
                return Ok(());
            }

            let index = Index::load(root_dir?, LoadMode::QuickCheck)?;

            if parsed_args.get_flag(0).is_some() {
                let query = index.uid_query(UidQuery::with_query(args[3].to_string()).file_or_chunk().no_staged_file())?;

                let processed_doc = if query.has_multiple_matches() {
                    return Err(Error::UidQueryError(format!("There're {} chunks/files that match `{}`. Please give more specific query.", query.len(), &args[3])));
                }

                else if query.is_empty() {
                    return Err(Error::UidQueryError(format!("There's no chunk or file that matches `{}`.", &args[3])));
                }

                else if let Some((_, uid)) = query.get_processed_file() {
                    index.get_tfidf_by_file_uid(uid)?
                }

                else if let Some(uid) = query.get_chunk_uid() {
                    index.get_tfidf_by_chunk_uid(uid)?
                }

                else {
                    unreachable!()
                };

                println!("{}", processed_doc.render());
                return Ok(());
            }

            let keywords = Keywords::from_raw(parsed_args.get_args());
            let tokenized_keywords = keywords.tokenize();
            let tfidf_results = index.run_tfidf(
                keywords,
                vec![],
                index.query_config.max_summaries,
            )?;
            let mut chunks = Vec::with_capacity(tfidf_results.len());

            for tfidf_result in tfidf_results.iter() {
                let (external_index, uid) = tfidf_result.id.clone();

                match external_index {
                    Some(i) => {
                        chunks.push(index.get_external_base(&i)?.get_chunk_by_uid(uid)?);
                    },
                    None => { chunks.push(index.get_chunk_by_uid(uid)?); },
                }
            }

            println!("search keywords: {:?}", parsed_args.get_args());
            println!("tokenized keywords: {:?}", tokenized_keywords.iter().map(|(token, _)| token).collect::<Vec<_>>());
            println!("found {} results", chunks.len());

            for (tfidf, chunk) in tfidf_results.iter().zip(chunks.iter()) {
                println!("--------------------------");
                println!("score: {}", tfidf.score);
                println!("file: {}", chunk.render_source());
                println!("title: {}", chunk.title);
                println!("summary: {}", chunk.summary);
            }
        },
        Some("version") => {
            let parsed_args = ArgParser::new().parse(&args[2..])?;

            if parsed_args.show_help() {
                println!("{}", include_str!("../docs/commands/version.txt"));
                return Ok(());
            }

            println!("ragit {}", ragit::VERSION);
        },
        // TODO: suggest similar names
        Some(invalid_command) => {
            return Err(Error::CliError(format!("`{invalid_command}` is an invalid command. Run `rag help` to get help.")));
        },
        None => {
            println!("Run `rag help` to get help.");
        },
    }

    Ok(())
}

// it starts from "." and goes up until it finds ".ragit"
// you can run git commands anywhere inside a repo, and I want ragit to be like that
fn find_root() -> Result<String, Error> {
    let mut curr = String::from(".");

    loop {
        let curr_files_ = read_dir(&curr)?;
        let mut curr_files = Vec::with_capacity(curr_files_.len());

        for f in curr_files_.iter() {
            curr_files.push(basename(f)?);
        }

        if curr_files.contains(&INDEX_DIR_NAME.to_string()) {
            return Ok(curr);
        }

        curr = join(&curr, "..")?;
    }
}
