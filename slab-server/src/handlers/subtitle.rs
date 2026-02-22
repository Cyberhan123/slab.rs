use axum::{http::StatusCode, Json};

use crate::api::subtitle;

#[axum::debug_handler]
pub async fn generate(
    // this argument tells axum to parse the request body
    // as JSON into a `CreateUser` type
    Json(payload): Json<subtitle::CreateUser>,
) -> (StatusCode, Json<subtitle::User>) {
    // insert your application logic here
    let user = subtitle::User {
        id: 1337,
        username: payload.username,
    };

    // this will be converted into a JSON response
    // with a status code of `201 Created`
    (StatusCode::CREATED, Json(user))
}


use anyhow::Result;

use std::path::{Path, PathBuf};
use subparse::{
    get_subtitle_format, parse_str, timetypes::TimeSpan, SrtFile, SubtitleEntry,
    SubtitleFileInterface,
};
pub struct SubtitleService {}

impl SubtitleService {
    pub fn new() -> Self {
        Self {}
    }

    fn read_file(path: &Path) -> String {
        use std::io::Read;
        let mut file = std::fs::File::open(path).unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        return s;
    }

    fn to_srt_entries(entries: Vec<SubtitleEntry>) -> Vec<(TimeSpan, String)> {
        entries
            .into_iter()
            .map(|entry| (entry.timespan, entry.line.unwrap_or_default()))
            .collect()
    }

    pub fn to_srt_string(&self, entries: Vec<SubtitleEntry>) -> Result<String> {
        let sub_file =
            SrtFile::create(Self::to_srt_entries(entries)).map_err(|e| anyhow::anyhow!(e))?;

        let buf = sub_file.to_data().map_err(|e| anyhow::anyhow!(e))?;
        Ok(String::from_utf8(buf)?)
    }

    pub fn to_srt_file<P: AsRef<Path>>(
        &self,
        file_path: P,
        entries: Vec<SubtitleEntry>,
    ) -> Result<()> {
        let srt_data = self.to_srt_string(entries)?;
        std::fs::write(file_path, srt_data)?;
        Ok(())
    }

    pub fn from_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SubtitleEntry>> {
        let file_content: String = Self::read_file(path.as_ref()); // your own load routine

        // parse the file
        let format = get_subtitle_format(path.as_ref().extension(), file_content.as_bytes())
            .ok_or_else(|| anyhow::anyhow!("unknown format"))?;

        let mut subtitle_file =
            parse_str(format, &file_content, 25.0).map_err(|e| anyhow::anyhow!(e))?;

        let mut subtitle_entries: Vec<SubtitleEntry> = subtitle_file
            .get_subtitle_entries()
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(subtitle_entries)
    }
}
