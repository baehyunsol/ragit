use std::collections::HashMap;

mod dist;
mod error;
mod span;

pub use dist::{get_closest_string, substr_edit_distance};
pub use error::{Error, ErrorKind};
pub use span::Span;

pub struct ArgParser {
    arg_count: ArgCount,
    arg_type: ArgType,
    flags: Vec<Flag>,

    // `--N=20`, `--prefix=rust`
    arg_flags: HashMap<String, ArgFlag>,

    // '-f' -> '--force'
    short_flags: HashMap<String, String>,
}

impl ArgParser {
    pub fn new() -> Self {
        ArgParser {
            arg_count: ArgCount::None,
            arg_type: ArgType::String,
            flags: vec![],
            arg_flags: HashMap::new(),
            short_flags: HashMap::new(),
        }
    }

    pub fn args(&mut self, arg_type: ArgType, arg_count: ArgCount) -> &mut Self {
        self.arg_type = arg_type;
        self.arg_count = arg_count;
        self
    }

    pub fn flag(&mut self, flags: &[&str]) -> &mut Self {
        self.flags.push(Flag {
            values: flags.iter().map(|flag| flag.to_string()).collect(),
            optional: false,
            default: None,
        });
        self
    }

    pub fn optional_flag(&mut self, flags: &[&str]) -> &mut Self {
        self.flags.push(Flag {
            values: flags.iter().map(|flag| flag.to_string()).collect(),
            optional: true,
            default: None,
        });
        self
    }

    pub fn arg_flag(&mut self, flag: &str, arg_type: ArgType) -> &mut Self {
        self.arg_flags.insert(flag.to_string(), ArgFlag { flag: flag.to_string(), optional: false, default: None, arg_type });
        self
    }

    pub fn optional_arg_flag(&mut self, flag: &str, arg_type: ArgType) -> &mut Self {
        self.arg_flags.insert(flag.to_string(), ArgFlag { flag: flag.to_string(), optional: true, default: None, arg_type });
        self
    }

    pub fn arg_flag_with_default(&mut self, flag: &str, default: &str, arg_type: ArgType) -> &mut Self {
        self.arg_flags.insert(flag.to_string(), ArgFlag { flag: flag.to_string(), optional: true, default: Some(default.to_string()), arg_type });
        self
    }

    pub fn short_flag(&mut self, flags: &[&str]) -> &mut Self {
        for flag in flags.iter() {
            let short_flag = flag.get(1..3).unwrap().to_string();

            if let Some(old) = self.short_flags.get(&short_flag) {
                panic!("{flag} and {old} have the same short name!")
            }

            self.short_flags.insert(short_flag, flag.to_string());
        }

        self
    }

    // the first flag is the default value
    pub fn flag_with_default(&mut self, flags: &[&str]) -> &mut Self {
        self.flags.push(Flag {
            values: flags.iter().map(|flag| flag.to_string()).collect(),
            optional: true,
            default: Some(0),
        });
        self
    }

    pub fn map_short_flag(&self, flag: &str) -> String {
        match self.short_flags.get(flag) {
            Some(f) => f.to_string(),
            None => flag.to_string(),
        }
    }

    pub fn parse(&self, raw_args: &[String]) -> Result<ParsedArgs, Error> {
        self.parse_worker(raw_args).map_err(
            |mut e| {
                e.span = e.span.render(raw_args);
                e
            }
        )
    }

    pub fn parse_worker(&self, raw_args: &[String]) -> Result<ParsedArgs, Error> {
        let mut args = vec![];
        let mut flags = vec![None; self.flags.len()];
        let mut arg_flags = HashMap::new();
        let mut expecting_flag_arg: Option<ArgFlag> = None;
        let mut no_more_flags = false;

        if raw_args.get(0).map(|arg| arg.as_str()) == Some("--help") {
            return Ok(ParsedArgs {
                raw_args: raw_args.to_vec(),
                args,
                flags: vec![],
                arg_flags,
                show_help: true,
            });
        }

        'raw_arg_loop: for (arg_index, raw_arg) in raw_args.iter().enumerate() {
            if raw_arg == "--" {
                if let Some(arg_flag) = expecting_flag_arg {
                    return Err(Error {
                        span: Span::End,
                        kind: ErrorKind::MissingArgument(arg_flag.flag.to_string(), arg_flag.arg_type),
                    });
                }

                no_more_flags = true;
                continue;
            }

            if let Some(arg_flag) = expecting_flag_arg {
                expecting_flag_arg = None;
                arg_flag.arg_type.parse(raw_arg, Span::Exact(arg_index))?;

                if let Some(_) = arg_flags.insert(arg_flag.flag.clone(), raw_arg.to_string()) {
                    return Err(Error {
                        span: Span::Exact(arg_index),
                        kind: ErrorKind::SameFlagMultipleTimes(
                            arg_flag.flag.clone(),
                            arg_flag.flag.clone(),
                        ),
                    });
                }

                continue;
            }

            if raw_arg.starts_with("-") && !no_more_flags {
                let mapped_flag = self.map_short_flag(raw_arg);

                for (flag_index, flag) in self.flags.iter().enumerate() {
                    if flag.values.contains(&mapped_flag) {
                        if flags[flag_index].is_none() {
                            flags[flag_index] = Some(mapped_flag.to_string());
                            continue 'raw_arg_loop;
                        }

                        else {
                            return Err(Error {
                                span: Span::Exact(arg_index),
                                kind: ErrorKind::SameFlagMultipleTimes(
                                    flags[flag_index].as_ref().unwrap().to_string(),
                                    raw_arg.to_string(),
                                ),
                            });
                        }
                    }
                }

                if let Some(arg_flag) = self.arg_flags.get(&mapped_flag) {
                    expecting_flag_arg = Some(arg_flag.clone());
                    continue;
                }

                if raw_arg.contains("=") {
                    let splitted = raw_arg.splitn(2, '=').collect::<Vec<_>>();
                    let flag = self.map_short_flag(splitted[0]);
                    let flag_arg = splitted[1];

                    if let Some(arg_flag) = self.arg_flags.get(&flag) {
                        arg_flag.arg_type.parse(flag_arg, Span::Exact(arg_index))?;

                        if let Some(_) = arg_flags.insert(flag.to_string(), flag_arg.to_string()) {
                            return Err(Error {
                                span: Span::Exact(arg_index),
                                kind: ErrorKind::SameFlagMultipleTimes(
                                    flag.to_string(),
                                    flag.to_string(),
                                ),
                            });
                        }

                        continue;
                    }

                    else {
                        return Err(Error {
                            span: Span::Exact(arg_index),
                            kind: ErrorKind::UnknownFlag {
                                flag: flag.to_string(),
                                similar_flag: self.get_similar_flag(&flag),
                            },
                        });
                    }
                }

                return Err(Error {
                    span: Span::Exact(arg_index),
                    kind: ErrorKind::UnknownFlag {
                        flag: raw_arg.to_string(),
                        similar_flag: self.get_similar_flag(raw_arg),
                    },
                });
            }

            else {
                args.push(self.arg_type.parse(raw_arg, Span::Exact(arg_index))?);
            }
        }

        if let Some(arg_flag) = expecting_flag_arg {
            return Err(Error {
                span: Span::End,
                kind: ErrorKind::MissingArgument(arg_flag.flag.to_string(), arg_flag.arg_type),
            });
        }

        for i in 0..flags.len() {
            if flags[i].is_none() {
                if let Some(j) = self.flags[i].default {
                    flags[i] = Some(self.flags[i].values[j].clone());
                }

                else if !self.flags[i].optional {
                    return Err(Error {
                        span: Span::End,
                        kind: ErrorKind::MissingFlag(self.flags[i].values.join(" | ")),
                    });
                }
            }
        }

        loop {
            let span = match self.arg_count {
                ArgCount::Geq(n) if args.len() < n => { Span::End },
                ArgCount::Leq(n) if args.len() > n => { Span::NthArg(n + 1) },
                ArgCount::Exact(n) if args.len() != n => { Span::NthArg(n + 1) },
                ArgCount::None if args.len() > 0 => { Span::FirstArg },
                _ => { break; },
            };

            return Err(Error {
                span,
                kind: ErrorKind::WrongArgCount {
                    expected: self.arg_count,
                    got: args.len(),
                },
            });
        }

        for (flag, arg_flag) in self.arg_flags.iter() {
            if arg_flags.contains_key(flag) {
                continue;
            }

            else if let Some(default) = &arg_flag.default {
                arg_flags.insert(flag.to_string(), default.to_string());
            }

            else if !arg_flag.optional {
                return Err(Error {
                    span: Span::End,
                    kind: ErrorKind::MissingFlag(flag.to_string()),
                });
            }
        }

        Ok(ParsedArgs {
            raw_args: raw_args.to_vec(),
            args,
            flags,
            arg_flags,
            show_help: false,
        })
    }

    fn get_similar_flag(&self, flag: &str) -> Option<String> {
        let mut candidates = vec![];

        for flag in self.flags.iter() {
            for flag in flag.values.iter() {
                candidates.push(flag.to_string());
            }
        }

        for flag in self.arg_flags.keys() {
            candidates.push(flag.to_string());
        }

        get_closest_string(&candidates, flag)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ArgCount {
    Geq(usize),
    Leq(usize),
    Exact(usize),
    Any,
    None,
}

#[derive(Clone, Copy, Debug)]
pub enum ArgType {
    String,
    Path,
    Command,
    Query,  // uid or path
    Integer,
    UnsignedInteger,
}

impl ArgType {
    pub fn parse(&self, arg: &str, span: Span) -> Result<String, Error> {
        match self {
            ArgType::Integer => match arg.parse::<i128>() {
                Ok(_) => Ok(arg.to_string()),
                Err(e) => Err(Error {
                    span,
                    kind: ErrorKind::ParseIntError(e),
                }),
            },
            ArgType::UnsignedInteger => match arg.parse::<u128>() {
                Ok(_) => Ok(arg.to_string()),
                Err(e) => Err(Error {
                    span,
                    kind: ErrorKind::ParseIntError(e),
                }),
            },
            ArgType::String
            | ArgType::Path
            | ArgType::Command  // TODO: validator for ArgType::Command
            | ArgType::Query => Ok(arg.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Flag {
    values: Vec<String>,
    optional: bool,
    default: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ArgFlag {
    flag: String,
    optional: bool,
    default: Option<String>,
    arg_type: ArgType,
}

pub struct ParsedArgs {
    raw_args: Vec<String>,
    args: Vec<String>,
    flags: Vec<Option<String>>,
    pub arg_flags: HashMap<String, String>,
    show_help: bool,  // TODO: options for help messages
}

impl ParsedArgs {
    pub fn get_args(&self) -> Vec<String> {
        self.args.clone()
    }

    pub fn get_args_exact(&self, count: usize) -> Result<Vec<String>, Error> {
        if self.args.len() == count {
            Ok(self.args.clone())
        }

        else {
            Err(Error {
                span: Span::FirstArg.render(&self.raw_args),
                kind: ErrorKind::WrongArgCount {
                    expected: ArgCount::Exact(count),
                    got: self.args.len(),
                },
            })
        }
    }

    // if there's an index error, it panics instead of returning None
    // if it returns None, that means Nth flag is optional and its value is None
    pub fn get_flag(&self, index: usize) -> Option<String> {
        self.flags[index].clone()
    }

    pub fn show_help(&self) -> bool {
        self.show_help
    }
}

pub fn underline_span(prefix: &str, args: &str, start: usize, end: usize) -> String {
    format!(
        "{prefix}{args}\n{}{}{}{}",
        " ".repeat(prefix.len()),
        " ".repeat(start),
        "^".repeat(end - start),
        " ".repeat(args.len() - end),
    )
}
