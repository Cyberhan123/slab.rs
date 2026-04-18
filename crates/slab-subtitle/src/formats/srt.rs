// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use self::errors::ErrorKind::*;
use self::errors::*;
use crate::{SubtitleEntry, SubtitleFileInterface};

use crate::errors::Result as SubtitleParserResult;
use crate::formats::common::*;
use combine::parser::char::{char, string};
use combine::{Parser, eof, skip_many};

use itertools::Itertools;

use crate::timetypes::{TimePoint, TimeSpan};
use std::iter::once;

type Result<T> = std::result::Result<T, Error>;

/// Errors specific to `.srt`-parsing.
#[allow(missing_docs)]
pub mod errors {

    define_error!(Error, ErrorKind);

    #[derive(PartialEq, Debug, thiserror::Error)]
    pub enum ErrorKind {
        #[error("expected SubRip index line, found '{line}'")]
        ExpectedIndexLine { line: String },

        #[error("expected SubRip timespan line, found '{line}'")]
        ExpectedTimestampLine { line: String },

        #[error("parse error at line `{line_num}`")]
        ErrorAtLine { line_num: usize },
    }
}

/// The parsing works as a finite state machine. These are the states in it.
enum SrtParserState {
    // emptyline or index follows
    Emptyline,

    /// timing line follows
    Index(i64),

    /// dialog or emptyline follows
    Timing(i64, TimeSpan),

    /// emptyline follows
    Dialog(i64, TimeSpan, Vec<String>),
}

#[derive(Debug, Clone)]
/// Represents a `.srt` file.
pub struct SrtFile {
    v: Vec<SrtLine>,
}

#[derive(Debug, Clone)]
/// A complete description of one `SubRip` subtitle line.
struct SrtLine {
    /// start and end time of subtitle
    timespan: TimeSpan,

    /// index/number of line
    index: i64,

    /// the dialog/text lines of the `SrtLine`
    texts: Vec<String>,
}

impl SrtFile {
    /// Parse a `.srt` subtitle string to `SrtFile`.
    pub fn parse(s: &str) -> SubtitleParserResult<SrtFile> {
        Self::parse_file(s)
            .map_err(|error| crate::Error::with_source(crate::ErrorKind::ParsingError, error))
    }
}

/// Implements parse functions.
impl SrtFile {
    fn parse_file(i: &str) -> Result<SrtFile> {
        use self::SrtParserState::*;

        let mut result: Vec<SrtLine> = Vec::new();

        // remove utf-8 bom
        let (_, s) = split_bom(i);

        let mut state: SrtParserState = Emptyline; // expect emptyline or index

        // the `once("")` is there so no last entry gets ignored
        for (line_num, line) in s.lines().chain(once("")).enumerate() {
            state = match state {
                Emptyline => {
                    if line.trim().is_empty() {
                        Emptyline
                    } else {
                        Index(Self::parse_index_line(line_num, line)?)
                    }
                }
                Index(index) => Timing(index, Self::parse_timespan_line(line_num, line)?),
                Timing(index, timespan) => {
                    Self::state_expect_dialog(line, &mut result, index, timespan, Vec::new())
                }
                Dialog(index, timespan, texts) => {
                    Self::state_expect_dialog(line, &mut result, index, timespan, texts)
                }
            };
        }

        Ok(SrtFile { v: result })
    }

    fn state_expect_dialog(
        line: &str,
        result: &mut Vec<SrtLine>,
        index: i64,
        timespan: TimeSpan,
        mut texts: Vec<String>,
    ) -> SrtParserState {
        if line.trim().is_empty() {
            result.push(SrtLine { index, timespan, texts });
            SrtParserState::Emptyline
        } else {
            texts.push(line.trim().to_string());
            SrtParserState::Dialog(index, timespan, texts)
        }
    }

    /// Matches a line with a single index.
    fn parse_index_line(line_num: usize, s: &str) -> Result<i64> {
        s.trim().parse::<i64>().map_err(|error| {
            Error::with_source(
                ErrorAtLine { line_num },
                Error::with_source(ExpectedIndexLine { line: s.to_string() }, error),
            )
        })
    }

    /// Matches a `SubRip` timespan like "00:24:45,670 --> 00:24:45,680".
    fn parse_timespan_line(line_num: usize, line: &str) -> Result<TimeSpan> {
        // Matches a `SubRip` timestamp like "00:24:45,670"
        let timestamp = || {
            (
                number_i64(),
                char(':'),
                number_i64(),
                char(':'),
                number_i64(),
                char(','),
                number_i64(),
            )
                .map(|t| TimePoint::from_components(t.0, t.2, t.4, t.6))
        };

        (
            skip_many(ws()),
            timestamp(),
            skip_many(ws()),
            string("-->"),
            skip_many(ws()),
            timestamp(),
            skip_many(ws()),
            eof(),
        )
            .map(|t| TimeSpan::new(t.1, t.5))
            .parse(line)
            .map(|(timespan, _)| timespan)
            .map_err(|error| {
                Error::with_source(
                    ErrorAtLine { line_num },
                    Error::with_source(ExpectedTimestampLine { line: line.to_string() }, error),
                )
            })
    }
}

impl SubtitleFileInterface for SrtFile {
    fn get_subtitle_entries(&self) -> SubtitleParserResult<Vec<SubtitleEntry>> {
        let timings = self
            .v
            .iter()
            .map(|line| SubtitleEntry::new(line.timespan, line.texts.iter().join("\n")))
            .collect();

        Ok(timings)
    }

    fn update_subtitle_entries(
        &mut self,
        new_subtitle_entries: &[SubtitleEntry],
    ) -> SubtitleParserResult<()> {
        assert_eq!(self.v.len(), new_subtitle_entries.len()); // required by specification of this function

        for (line_ref, new_entry_ref) in self.v.iter_mut().zip(new_subtitle_entries) {
            line_ref.timespan = new_entry_ref.timespan;
            if let Some(ref text) = new_entry_ref.line {
                line_ref.texts = text.lines().map(str::to_string).collect();
            }
        }

        Ok(())
    }

    fn to_data(&self) -> SubtitleParserResult<Vec<u8>> {
        let timepoint_to_str = |t: TimePoint| -> String {
            format!(
                "{:02}:{:02}:{:02},{:03}",
                t.hours(),
                t.mins_comp(),
                t.secs_comp(),
                t.msecs_comp()
            )
        };
        let line_to_str = |line: &SrtLine| -> String {
            format!(
                "{}\n{} --> {}\n{}\n\n",
                line.index,
                timepoint_to_str(line.timespan.start),
                timepoint_to_str(line.timespan.end),
                line.texts.join("\n")
            )
        };

        Ok(self.v.iter().map(line_to_str).collect::<String>().into_bytes())
    }
}

impl SrtFile {
    /// Creates .srt file from scratch.
    pub fn create(v: Vec<(TimeSpan, String)>) -> SubtitleParserResult<SrtFile> {
        let file_parts = v
            .into_iter()
            .enumerate()
            .map(|(i, (ts, text))| SrtLine {
                index: i as i64 + 1,
                timespan: ts,
                texts: text.lines().map(str::to_string).collect(),
            })
            .collect();

        Ok(SrtFile { v: file_parts })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_srt_test() {
        use crate::SubtitleFileInterface;
        use crate::timetypes::{TimePoint, TimeSpan};

        let lines = vec![
            (
                TimeSpan::new(TimePoint::from_msecs(1500), TimePoint::from_msecs(3700)),
                "line1".to_string(),
            ),
            (
                TimeSpan::new(TimePoint::from_msecs(4500), TimePoint::from_msecs(8700)),
                "line2".to_string(),
            ),
        ];
        let file = super::SrtFile::create(lines).unwrap();

        // generate file
        let data_string = String::from_utf8(file.to_data().unwrap()).unwrap();
        let expected = "1\n00:00:01,500 --> 00:00:03,700\nline1\n\n2\n00:00:04,500 --> 00:00:08,700\nline2\n\n"
            .to_string();
        println!("\n{:?}\n{:?}", data_string, expected);
        assert_eq!(data_string, expected);
    }

    #[test]
    fn parse_valid_srt_file() {
        use crate::SubtitleFileInterface;

        let srt_content = "1\n00:00:01,000 --> 00:00:04,000\nFirst subtitle\n\n2\n00:00:05,500 --> 00:00:08,000\nSecond subtitle\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse valid SRT");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].line.as_deref(), Some("First subtitle"));
        assert_eq!(entries[1].line.as_deref(), Some("Second subtitle"));
    }

    #[test]
    fn parse_srt_with_multiline_text() {
        use crate::SubtitleFileInterface;

        let srt_content = "1\n00:00:01,000 --> 00:00:04,000\nLine 1\nLine 2\nLine 3\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse multiline SRT");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].line.as_deref(),
            Some("Line 1\nLine 2\nLine 3")
        );
    }

    #[test]
    fn parse_srt_handles_empty_lines() {
        use crate::SubtitleFileInterface;

        let srt_content = "\n\n1\n00:00:01,000 --> 00:00:04,000\nText\n\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse SRT with leading empty lines");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn parse_srt_handles_bom() {
        use crate::SubtitleFileInterface;

        let srt_content = "\u{FEFF}1\n00:00:01,000 --> 00:00:04,000\nText\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse SRT with BOM");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn parse_srt_invalid_index_fails() {
        let srt_content = "invalid\n00:00:01,000 --> 00:00:04,000\nText\n\n";

        let result = SrtFile::parse(srt_content);
        assert!(result.is_err(), "should fail with invalid index");
    }

    #[test]
    fn parse_srt_invalid_timestamp_format_fails() {
        let srt_content = "1\ninvalid timestamp --> 00:00:04,000\nText\n\n";

        let result = SrtFile::parse(srt_content);
        assert!(
            result.is_err(),
            "should fail with invalid timestamp format"
        );
    }

    #[test]
    fn parse_srt_empty_content() {
        let srt_content = "";

        let file = SrtFile::parse(srt_content).expect("should parse empty SRT");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn parse_srt_only_empty_lines() {
        let srt_content = "\n\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse SRT with only empty lines");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn parse_srt_malformed_timestamp_missing_arrow() {
        let srt_content = "1\n00:00:01,000 00:00:04,000\nText\n\n";

        let result = SrtFile::parse(srt_content);
        assert!(result.is_err(), "should fail with missing timestamp arrow");
    }

    #[test]
    fn parse_srt_with_special_characters() {
        use crate::SubtitleFileInterface;

        let srt_content = "1\n00:00:01,000 --> 00:00:04,000\nSpecial chars: <>&\"'\\`\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse SRT with special chars");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].line.as_deref(),
            Some("Special chars: <>&\"'\\`")
        );
    }

    #[test]
    fn parse_srt_with_unicode() {
        use crate::SubtitleFileInterface;

        let srt_content = "1\n00:00:01,000 --> 00:00:04,000\nUnicode: 你好世界 🌍\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse SRT with Unicode");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].line.as_deref(), Some("Unicode: 你好世界 🌍"));
    }

    #[test]
    fn parse_srt_very_long_timestamp() {
        use crate::SubtitleFileInterface;

        let srt_content = "1\n99:59:59,999 --> 99:59:59,999\nLong timestamp\n\n";

        let file = SrtFile::parse(srt_content).expect("should parse SRT with long timestamp");
        let entries = file.get_subtitle_entries().expect("should get entries");

        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn update_subtitle_entries_preserves_non_text() {
        use crate::SubtitleFileInterface;
        use crate::timetypes::{TimePoint, TimeSpan};

        let srt_content = "1\n00:00:01,000 --> 00:00:04,000\nOriginal text\n\n";

        let mut file = SrtFile::parse(srt_content).expect("should parse valid SRT");

        let new_entries = vec![SubtitleEntry {
            timespan: TimeSpan::new(TimePoint::from_msecs(2000), TimePoint::from_msecs(5000)),
            line: Some("Updated text".to_string()),
        }];

        let result = file.update_subtitle_entries(&new_entries);
        assert!(result.is_ok(), "should update entries");

        let entries = file.get_subtitle_entries().expect("should get entries");
        assert_eq!(entries[0].line.as_deref(), Some("Updated text"));
    }

    #[test]
    fn to_data_generates_valid_srt_format() {
        use crate::SubtitleFileInterface;
        use crate::timetypes::{TimePoint, TimeSpan};

        let lines = vec![(
            TimeSpan::new(TimePoint::from_msecs(1000), TimePoint::from_msecs(4000)),
            "Test subtitle".to_string(),
        )];

        let file = SrtFile::create(lines).expect("should create SRT file");
        let data = file.to_data().expect("should serialize");
        let content = String::from_utf8(data).expect("should be valid UTF-8");

        assert!(content.contains("1\n"));
        assert!(content.contains("00:00:01,000 --> 00:00:04,000\n"));
        assert!(content.contains("Test subtitle"));
    }
}
// Parser tests completed
