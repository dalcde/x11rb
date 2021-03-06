use std::cell::RefCell;
use std::path::PathBuf;

use fxhash::FxHashMap;

#[macro_use]
mod output;
mod error_events;
mod namespace;
mod special_cases;

use output::Output;

pub(crate) fn generate(module: &xcbgen::defs::Module) -> FxHashMap<PathBuf, String> {
    let mut out_map = FxHashMap::default();

    let mut main_out = Output::new();
    write_code_header(&mut main_out);
    outln!(main_out, "use std::convert::{{TryFrom, TryInto}};");
    outln!(main_out, "use crate::errors::ParseError;");
    outln!(main_out, "use crate::x11_utils::ExtInfoProvider;");
    outln!(main_out, "");

    let caches = RefCell::new(namespace::Caches::default());
    for ns in module.sorted_namespaces() {
        let mut ns_out = Output::new();
        namespace::generate(&ns, &caches, &mut ns_out);
        out_map.insert(
            PathBuf::from(format!("{}.rs", ns.header)),
            ns_out.into_data(),
        );

        if ext_has_feature(&ns.header) {
            outln!(main_out, "#[cfg(feature = \"{}\")]", ns.header);
        }
        outln!(main_out, "pub mod {};", ns.header);
    }
    outln!(main_out, "");

    error_events::generate(&mut main_out, module);

    out_map.insert(PathBuf::from("mod.rs"), main_out.into_data());
    out_map
}

fn ext_has_feature(name: &str) -> bool {
    match name {
        "bigreq" | "ge" | "xc_misc" | "xproto" => false,
        _ => true,
    }
}

/// Add a Rust-header to the output saying that this file is generated.
fn write_code_header(out: &mut Output) {
    outln!(
        out,
        "// This file contains generated code. Do not edit directly.",
    );
    outln!(out, "// To regenerate this, run 'make'.");
    outln!(out, "");
}

fn camel_case_to_snake(arg: &str) -> String {
    assert!(
        arg.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'),
        "{:?}",
        arg
    );

    // Matches "[A-Z][a-z0-9]+|[A-Z]+(?![a-z0-9])|[a-z0-9]+"
    struct Matcher<'a> {
        remaining: &'a str,
    }

    impl<'a> Matcher<'a> {
        fn new(s: &'a str) -> Self {
            Self { remaining: s }
        }
    }

    impl<'a> Iterator for Matcher<'a> {
        type Item = &'a str;

        fn next(&mut self) -> Option<&'a str> {
            enum State {
                Begin,
                // "[A-Z]"
                OneUpper(usize),
                // "[A-Z][a-z0-9]+"
                OneUpperThenLowerOrDigit(usize),
                // "[A-Z][A-Z]+"
                ManyUpper(usize),
                // "[a-z0-9]+"
                LowerOrDigit(usize),
            }

            let s = self.remaining;
            let mut chr_iter = s.char_indices();
            let mut state = State::Begin;
            let next_match = loop {
                let (chr_i, chr) = chr_iter
                    .next()
                    .map(|(chr_i, chr)| (chr_i, Some(chr)))
                    .unwrap_or((s.len(), None));
                match state {
                    State::Begin => match chr {
                        None => break None,
                        Some('A'..='Z') => state = State::OneUpper(chr_i),
                        Some('a'..='z') | Some('0'..='9') => state = State::LowerOrDigit(chr_i),
                        Some(_) => state = State::Begin,
                    },
                    State::OneUpper(begin_i) => match chr {
                        Some('A'..='Z') => state = State::ManyUpper(begin_i),
                        Some('a'..='z') | Some('0'..='9') => {
                            state = State::OneUpperThenLowerOrDigit(begin_i)
                        }
                        _ => break Some((&s[begin_i..chr_i], &s[chr_i..])),
                    },
                    State::OneUpperThenLowerOrDigit(begin_i) => match chr {
                        Some('a'..='z') | Some('0'..='9') => {
                            state = State::OneUpperThenLowerOrDigit(begin_i)
                        }
                        _ => break Some((&s[begin_i..chr_i], &s[chr_i..])),
                    },
                    State::ManyUpper(begin_i) => match chr {
                        Some('A'..='Z') => state = State::ManyUpper(begin_i),
                        Some('a'..='z') | Some('0'..='9') => {
                            break Some((&s[begin_i..(chr_i - 1)], &s[(chr_i - 1)..]));
                        }
                        _ => break Some((&s[begin_i..chr_i], &s[chr_i..])),
                    },
                    State::LowerOrDigit(begin_i) => match chr {
                        Some('a'..='z') | Some('0'..='9') => state = State::LowerOrDigit(begin_i),
                        _ => break Some((&s[begin_i..chr_i], &s[chr_i..])),
                    },
                }
            };

            if let Some((match_str, remaining)) = next_match {
                self.remaining = remaining;
                Some(match_str)
            } else {
                None
            }
        }
    }

    let mut r = String::new();
    for match_str in Matcher::new(arg) {
        if !r.is_empty() {
            r.push('_');
        }
        r.push_str(match_str);
    }
    r
}

fn camel_case_to_lower_snake(arg: &str) -> String {
    let mut r = camel_case_to_snake(arg);
    r.make_ascii_lowercase();
    r
}

fn camel_case_to_upper_snake(arg: &str) -> String {
    let mut r = camel_case_to_snake(arg);
    r.make_ascii_uppercase();
    r
}
