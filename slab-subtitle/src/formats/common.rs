// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use combine::parser::char::{char, digit};
use combine::parser::token::{any, eof, satisfy};
use combine::{many, many1, optional, Parser, Stream};
use std::fmt::Display;
use std::str::FromStr;

/// Returns the string without BOMs. Unchanged if string does not start with one.
pub fn split_bom(s: &str) -> (&str, &str) {
    if s.starts_with('\u{FEFF}') {
        s.split_at(3)
    } else {
        ("", s)
    }
}

#[test]

fn test_split_bom() {
    let bom1 = "\u{FEFF}";
    let input_with_bom = "\u{FEFF}abc";
    assert_eq!(split_bom(input_with_bom), (bom1, "abc"));
    assert_eq!(split_bom("bla"), ("", "bla"));
    assert_eq!(split_bom(""), ("", ""));
}

/// Parses whitespaces and tabs.
#[inline]
pub fn ws<I>() -> impl Parser<I, Output = char>
where
    I: Stream<Token = char>,
{
    satisfy(|c: char| c == ' ' || c == '\t').expected("tab or space")
}

/// Matches a positive or negative intger number.
pub fn number_i64<I>() -> impl Parser<I, Output = i64>
where
    I: Stream<Token = char>,
{
    (optional(char('-')), many1(digit()))
        .map(|(a, c): (Option<_>, String)| {
            // we provide a string that only contains digits: this unwrap should never fail
            let i: i64 = FromStr::from_str(&c).unwrap();
            match a {
                Some(_) => -i,
                None => i,
            }
        })
        .expected("positive or negative number")
}

/// Create a single-line-error string from a combine parser error.
pub fn parse_error_to_string<E>(p: E) -> String
where
    E: Display,
{
    p.to_string().trim().lines().map(str::trim).filter(|line| !line.is_empty()).fold(
        String::new(),
        |a: String, b: &str| {
            if a.is_empty() {
                b.to_string()
            } else {
                a + "; " + b
            }
        },
    )
}

/// This function does a very common task for non-destructive parsers: merging mergable consecutive file parts.
///
/// Each file has some "filler"-parts in it (unimportant information) which only get stored to reconstruct the
/// original file. Two consecutive filler parts (their strings) can be merged. This function abstracts over the
/// specific file part type.
pub fn dedup_string_parts<T, F>(v: Vec<T>, mut extract_fn: F) -> Vec<T>
where
    F: FnMut(&mut T) -> Option<&mut String>,
{
    let mut result = Vec::new();
    for mut part in v {
        let mut push_part = true;
        if let Some(last_part) = result.last_mut() {
            if let Some(exchangeable_text) = extract_fn(last_part) {
                if let Some(new_text) = extract_fn(&mut part) {
                    exchangeable_text.push_str(new_text);
                    push_part = false;
                }
            }
        }

        if push_part {
            result.push(part);
        }
    }

    result
}

// used in `get_lines_non_destructive()`
type SplittedLine = (String /* string */, String /* newline string like \n or \r\n */);

/// Iterates over all lines in `s` and calls the `process_line` closure for every line and line ending.
/// This ensures that we can reconstruct the file with correct line endings.
///
/// This will also accept the line ending `\r` (not within `\r\n`) to avoid error handling.
pub fn get_lines_non_destructive(s: &str) -> Vec<SplittedLine> {
    let mut result = Vec::new();
    let mut rest = s;
    loop {
        if rest.is_empty() {
            return result;
        }

        match rest.char_indices().find(|&(_, c)| c == '\r' || c == '\n') {
            Some((idx, _)) => {
                let (line_str, new_rest) = rest.split_at(idx);
                rest = new_rest;

                let line = line_str.to_string();
                if rest.starts_with("\r\n") {
                    result.push((line, "\r\n".to_string()));
                    rest = &rest[2..];
                } else if rest.starts_with('\n') {
                    result.push((line, "\n".to_string()));
                    rest = &rest[1..];
                } else if rest.starts_with('\r') {
                    // we only treat this as valid line ending to avoid error handling
                    result.push((line, "\r".to_string()));
                    rest = &rest[1..];
                }
            }
            None => {
                result.push((rest.to_string(), "".to_string()));
                return result;
            }
        }
    }
}

#[test]
fn get_lines_non_destructive_test0() {
    let lines = ["", "aaabb", "aaabb\r\nbcccc\n\r\n ", "aaabb\r\nbcccc"];
    for &full_line in lines.iter() {
        let joined: String = get_lines_non_destructive(full_line)
            .into_iter()
            .flat_map(|(s1, s2)| vec![s1, s2].into_iter())
            .collect();
        assert_eq!(full_line, joined);
    }
}

/// Trim a string left and right, but also preserve the white-space characters. The
/// seconds element in the returned tuple contains the non-whitespace string.
pub fn trim_non_destructive(s: &str) -> (String, String, String) {
    let (begin, rest) = trim_left(s);
    let (end, rest2) = trim_left(&rest.chars().rev().collect::<String>());
    (begin, rest2.chars().rev().collect(), end.chars().rev().collect())
}

/// Splits a string in whitespace string and the rest "   hello " -> ("   ", "hello ").
fn trim_left(s: &str) -> (String, String) {
    (many(ws()), many(any()), eof())
        .map(|t: (String, String, ())| (t.0, t.1))
        .parse(s)
        .expect("the trim parser should accept any input")
        .0
}
