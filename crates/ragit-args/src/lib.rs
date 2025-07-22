use std::collections::HashMap;

pub struct Error {
    pub span: Span,
    pub kind: ErrorKind,
}

pub enum ErrorKind {
    /// see <https://doc.rust-lang.org/stable/std/num/struct.ParseIntError.html>
    ParseIntError(std::num::ParseIntError),

    IntegerNotInRange {
        min: Option<i128>,
        max: Option<i128>,
        n: i128,
    },

    /// (prev_flag, curr_flag)
    SameFlagMultipleTimes(String, String),

    /// of an arg_flag
    MissingArgument(String, ArgType),

    WrongArgCount {
        expected: ArgCount,
        got: usize,
    },
    MissingFlag(String),
    UnknownFlag {
        flag: String,
        similar_flag: Option<String>,
    },
}

impl ErrorKind {
    pub fn render(&self) -> String {
        match self {
            ErrorKind::ParseIntError(_) => String::from("Cannot parse int."),
            ErrorKind::IntegerNotInRange { min, max, n } => match (min, max) {
                (Some(min), Some(max)) => format!("N is supposed to be between {min} and {max}, but is {n}."),
                (Some(min), None) => format!("N is supposed to be at least {min}, but is {n}."),
                (None, Some(max)) => format!("N is supposed to be at most {max}, but is {n}."),
                (None, None) => unreachable!(),
            },
            ErrorKind::SameFlagMultipleTimes(prev, next) => if prev == next {
                format!("Flag `{next}` cannot be used multiple times.")
            } else {
                format!("Flag `{prev}` and `{next}` cannot be used together.")
            },
            ErrorKind::MissingArgument(arg, arg_type) => format!(
                "A {} value is required for flag `{arg}`, but is missing.",
                format!("{arg_type:?}").to_ascii_lowercase(),
            ),
            ErrorKind::WrongArgCount { expected, got } => format!(
                "Expected {} arguments, got {got} arguments",
                match expected {
                    ArgCount::Exact(n) => format!("exactly {n}"),
                    ArgCount::Geq(n) => format!("at least {n}"),
                    ArgCount::Leq(n) => format!("at most {n}"),
                    ArgCount::None => String::from("no"),
                    ArgCount::Any => unreachable!(),
                },
            ),
            ErrorKind::MissingFlag(flag) => format!("Flag `{flag}` is missing."),
            ErrorKind::UnknownFlag { flag, similar_flag } => format!(
                "Unknown flag: `{flag}`.{}",
                if let Some(flag) = similar_flag {
                    format!(" There is a similar flag: `{flag}`.")
                } else {
                    String::new()
                },
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Span {
    Exact(usize),  // including flags and args
    FirstArg,
    End,
    NthArg(usize),  // including args, not including flags
    Rendered((String, usize, usize)),
}

impl Span {
    pub fn render(&self, args: &[String], skip_first_n: usize) -> Self {
        let mut rendered_args = Vec::with_capacity(args.len());
        let mut arg_indices = vec![];

        for (index, arg) in args.iter().enumerate() {
            if !arg.starts_with("--") && index >= skip_first_n {
                arg_indices.push(index);
            }

            if arg.contains(" ") || arg.contains("\"") || arg.contains("'") || arg.contains("\n") {
                rendered_args.push(format!("{arg:?}"));
            }

            else {
                rendered_args.push(arg.to_string());
            }
        }

        let new_span = match self {
            Span::Exact(n) => Span::Exact(*n),
            Span::FirstArg => match arg_indices.get(0) {
                Some(n) => Span::Exact(*n),
                None => Span::End,
            },
            Span::NthArg(n) => match arg_indices.get(*n) {
                Some(n) => Span::Exact(*n),
                None => Span::End,
            },
            _ => self.clone(),
        };
        let selected_index = match new_span {
            Span::Exact(n) => n,
            _ => 0,
        };
        let mut joined_args = rendered_args.join(" ");
        let (start, end) = if joined_args.is_empty() {
            joined_args = String::from(" ");
            (0, 1)
        } else {
            // append a whitespace so that `Span::End` is more readable
            joined_args = format!("{joined_args} ");

            match new_span {
                Span::End => (joined_args.len() - 1, joined_args.len()),
                _ => (
                    rendered_args[..selected_index].iter().map(|arg| arg.len()).sum::<usize>() + selected_index,
                    rendered_args[..(selected_index + 1)].iter().map(|arg| arg.len()).sum::<usize>() + selected_index,
                ),
            }
        };

        Span::Rendered((
            joined_args,
            start,
            end,
        ))
    }

    pub fn unwrap_rendered(&self) -> (String, usize, usize) {
        match self {
            Span::Rendered((span, start, end)) => (span.to_string(), *start, *end),
            _ => panic!(),
        }
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

/// `ArgType` doesn't do much. Only `Integer` and
/// `IntegerBetween` variants do extra checks. The
/// other variants are more like type signatures
/// which clarifies the intent of the code.
#[derive(Clone, Copy, Debug)]
pub enum ArgType {
    String,
    Path,
    Command,
    Query,
    UidOrPath,
    Integer,
    Url,

    /// Both inclusive
    IntegerBetween {
        min: Option<i128>,
        max: Option<i128>,
    },
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
            ArgType::IntegerBetween { min, max } => match arg.parse::<i128>() {
                Ok(n) => {
                    if let Some(min) = *min {
                        if n < min {
                            return Err(Error{
                                span,
                                kind: ErrorKind::IntegerNotInRange { min: Some(min), max: *max, n },
                            });
                        }
                    }

                    if let Some(max) = *max {
                        if n > max {
                            return Err(Error{
                                span,
                                kind: ErrorKind::IntegerNotInRange { min: *min, max: Some(max), n },
                            });
                        }
                    }

                    Ok(arg.to_string())
                },
                Err(e) => Err(Error {
                    span,
                    kind: ErrorKind::ParseIntError(e),
                }),
            },
            ArgType::String
            | ArgType::Path
            | ArgType::Url
            | ArgType::UidOrPath
            | ArgType::Command
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

#[derive(Debug, Clone, Default)]
pub struct ParsedArgs {
    pub args: Vec<String>,
    pub arg_flags: HashMap<String, String>,
    pub flags: Vec<Option<String>>,
    pub show_help: bool,
    pub raw_args: Vec<String>,
    pub skip_first_n: usize,
}

impl ParsedArgs {
    pub fn new() -> Self {
        ParsedArgs::default()
    }

    pub fn new_help(raw_args: Vec<String>, skip_first_n: usize) -> Self {
        ParsedArgs {
            raw_args,
            skip_first_n,
            show_help: true,
            ..ParsedArgs::default()
        }
    }

    pub fn new_parsed(raw_args: Vec<String>, skip_first_n: usize, args: Vec<String>, flags: Vec<Option<String>>, arg_flags: HashMap<String, String>) -> Self {
        ParsedArgs {
            raw_args,
            skip_first_n,
            args,
            flags,
            arg_flags,
            show_help: false,
        }
    }

    pub fn get_args(&self) -> Vec<String> {
        self.args.clone()
    }

    pub fn get_args_exact(&self, count: usize) -> Result<Vec<String>, Error> {
        if self.args.len() == count {
            Ok(self.args.clone())
        }

        else {
            Err(Error {
                span: Span::FirstArg.render(&self.raw_args, self.skip_first_n),
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

pub struct ArgParser {
    arg_count: ArgCount,
    arg_type: ArgType,
    flags: Vec<Flag>,
    aliases: HashMap<String, String>,

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
            aliases: HashMap::new(),
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

    // the first flag is the default value
    pub fn flag_with_default(&mut self, flags: &[&str]) -> &mut Self {
        self.flags.push(Flag {
            values: flags.iter().map(|flag| flag.to_string()).collect(),
            optional: true,
            default: Some(0),
        });
        self
    }

    fn map_short_flag(&self, flag: &str) -> String {
        match self.short_flags.get(flag) {
            Some(f) => f.to_string(),
            None => flag.to_string(),
        }
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

    pub fn alias(&mut self, from: &str, to: &str) -> &mut Self {
        self.aliases.insert(from.to_string(), to.to_string());
        self
    }

    /// Let's say `raw_args` is `["rag", "ls-files", "--json", "--staged", "--name-only"]` and
    /// you don't care about the first 2 args (path and command name). You only want to parse
    /// the flags (the last 3 args). In this case, you set `skip_first_n` to 2.
    pub fn parse(&self, raw_args: &[String], skip_first_n: usize) -> Result<ParsedArgs, Error> {
        self.parse_worker(raw_args, skip_first_n).map_err(
            |mut e| {
                e.span = e.span.render(raw_args, skip_first_n);
                e
            }
        )
    }

    fn parse_worker(&self, raw_args: &[String], skip_first_n: usize) -> Result<ParsedArgs, Error> {
        let mut args = vec![];
        let mut flags = vec![None; self.flags.len()];
        let mut arg_flags = HashMap::new();
        let mut expecting_flag_arg: Option<ArgFlag> = None;
        let mut no_more_flags = false;

        if raw_args.get(skip_first_n).map(|arg| arg.as_str()) == Some("--help") {
            return Ok(ParsedArgs::new_help(raw_args.to_vec(), skip_first_n));
        }

        'raw_arg_loop: for (arg_index, raw_arg) in raw_args[skip_first_n..].iter().enumerate() {
            let raw_arg = match self.aliases.get(raw_arg) {
                Some(alias) => alias.to_string(),
                None => raw_arg.to_string(),
            };

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
                arg_flag.arg_type.parse(&raw_arg, Span::Exact(arg_index + skip_first_n))?;

                if let Some(_) = arg_flags.insert(arg_flag.flag.clone(), raw_arg.to_string()) {
                    return Err(Error {
                        span: Span::Exact(arg_index + skip_first_n),
                        kind: ErrorKind::SameFlagMultipleTimes(
                            arg_flag.flag.clone(),
                            arg_flag.flag.clone(),
                        ),
                    });
                }

                continue;
            }

            if raw_arg.starts_with("-") && !no_more_flags {
                let mapped_flag = self.map_short_flag(&raw_arg);

                for (flag_index, flag) in self.flags.iter().enumerate() {
                    if flag.values.contains(&mapped_flag) {
                        if flags[flag_index].is_none() {
                            flags[flag_index] = Some(mapped_flag.to_string());
                            continue 'raw_arg_loop;
                        }

                        else {
                            return Err(Error {
                                span: Span::Exact(arg_index + skip_first_n),
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
                        arg_flag.arg_type.parse(flag_arg, Span::Exact(arg_index + skip_first_n))?;

                        if let Some(_) = arg_flags.insert(flag.to_string(), flag_arg.to_string()) {
                            return Err(Error {
                                span: Span::Exact(arg_index + skip_first_n),
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
                            span: Span::Exact(arg_index + skip_first_n),
                            kind: ErrorKind::UnknownFlag {
                                flag: flag.to_string(),
                                similar_flag: None,
                            },
                        });
                    }
                }

                return Err(Error {
                    span: Span::Exact(arg_index + skip_first_n),
                    kind: ErrorKind::UnknownFlag {
                        flag: raw_arg.to_string(),
                        similar_flag: None,
                    },
                });
            }

            else {
                args.push(self.arg_type.parse(&raw_arg, Span::Exact(arg_index + skip_first_n))?);
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
                ArgCount::Exact(n) if args.len() > n => { Span::NthArg(n + 1) },
                ArgCount::Exact(n) if args.len() < n => { Span::NthArg(args.len().max(1) - 1) },
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

        Ok(ParsedArgs::new_parsed(raw_args.to_vec(), skip_first_n, args, flags, arg_flags))
    }
}
