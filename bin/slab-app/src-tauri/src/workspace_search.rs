use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::workspace::{
    MAX_FILE_BYTES, MAX_SEARCH_RESULTS, WorkspaceState, active_workspace, join_relative_path,
    should_hide_entry,
};

const MAX_LINE_MATCHES_PER_FILE: usize = 20;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextSearchResponse {
    pub query: String,
    pub matches: Vec<WorkspaceTextSearchFileMatch>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextSearchFileMatch {
    pub relative_path: String,
    pub name: String,
    pub line_matches: Vec<WorkspaceTextSearchLineMatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTextSearchLineMatch {
    pub line_number: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[tauri::command]
pub fn workspace_search_text(
    state: State<'_, WorkspaceState>,
    query: String,
) -> Result<WorkspaceTextSearchResponse, String> {
    let workspace = active_workspace(&state)?;
    search_workspace_text(Path::new(&workspace.root_path), &query)
}

fn search_workspace_text(root: &Path, query: &str) -> Result<WorkspaceTextSearchResponse, String> {
    let query = query.trim().to_owned();
    if query.is_empty() {
        return Ok(WorkspaceTextSearchResponse { query, matches: Vec::new(), truncated: false });
    }

    let search_query = query.to_ascii_lowercase();
    let mut matches = Vec::new();
    let mut truncated = false;
    search_directory(root, "", &search_query, &mut matches, &mut truncated)?;

    Ok(WorkspaceTextSearchResponse { query, matches, truncated })
}

fn search_directory(
    directory: &Path,
    relative_path: &str,
    query: &str,
    matches: &mut Vec<WorkspaceTextSearchFileMatch>,
    truncated: &mut bool,
) -> Result<(), String> {
    if matches.len() >= MAX_SEARCH_RESULTS {
        *truncated = true;
        return Ok(());
    }

    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to read directory {}: {error}", directory.display()))?
    {
        if matches.len() >= MAX_SEARCH_RESULTS {
            *truncated = true;
            break;
        }

        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let file_type =
            entry.file_type().map_err(|error| format!("failed to read file type: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_hide_entry(&name, file_type.is_dir(), false) {
            continue;
        }

        let entry_relative_path = join_relative_path(relative_path, &name);
        if file_type.is_dir() {
            search_directory(&entry.path(), &entry_relative_path, query, matches, truncated)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        if let Some(file_match) =
            search_file(&entry.path(), &entry_relative_path, &name, query, truncated)?
        {
            matches.push(file_match);
            if matches.len() >= MAX_SEARCH_RESULTS {
                *truncated = true;
                break;
            }
        }
    }

    Ok(())
}

fn search_file(
    path: &Path,
    relative_path: &str,
    name: &str,
    query: &str,
    truncated: &mut bool,
) -> Result<Option<WorkspaceTextSearchFileMatch>, String> {
    let Some(content) = read_searchable_file(path)? else {
        return Ok(None);
    };

    let mut line_matches = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let lower_line = line.to_ascii_lowercase();
        let Some(match_byte_start) = lower_line.find(query) else {
            continue;
        };
        let match_byte_end = match_byte_start + query.len();
        line_matches.push(WorkspaceTextSearchLineMatch {
            line_number: index + 1,
            line_text: line.to_owned(),
            match_start: line[..match_byte_start].chars().count(),
            match_end: line[..match_byte_end].chars().count(),
        });

        if line_matches.len() >= MAX_LINE_MATCHES_PER_FILE {
            *truncated = true;
            break;
        }
    }

    if line_matches.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceTextSearchFileMatch {
        relative_path: relative_path.to_owned(),
        name: name.to_owned(),
        line_matches,
    }))
}

fn read_searchable_file(path: &Path) -> Result<Option<String>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to read file metadata {}: {error}", path.display()))?;
    if metadata.len() > MAX_FILE_BYTES {
        return Ok(None);
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .map_err(|error| format!("failed to open file {}: {error}", path.display()))?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read file {}: {error}", path.display()))?;
    if bytes.contains(&0) {
        return Ok(None);
    }

    Ok(String::from_utf8(bytes).ok())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::search_workspace_text;

    #[test]
    fn text_search_skips_binary_and_ignored_directories() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("slab-workspace-text-search-{}-{suffix}", std::process::id()));
        fs::create_dir_all(root.join("src")).expect("create source directory");
        fs::create_dir_all(root.join("node_modules")).expect("create ignored directory");
        fs::write(root.join("src").join("main.rs"), "fn main() {\n  let Value = 1;\n}\n")
            .expect("write source file");
        fs::write(root.join("node_modules").join("ignored.rs"), "let Value = 2;\n")
            .expect("write ignored file");
        fs::write(root.join("binary.bin"), b"Value\0").expect("write binary file");

        let response = search_workspace_text(&root, "value").expect("search text");
        fs::remove_dir_all(root).expect("remove temp search fixture");

        assert!(!response.truncated);
        assert_eq!(response.matches.len(), 1);
        assert_eq!(response.matches[0].relative_path, "src/main.rs");
        assert_eq!(response.matches[0].line_matches[0].line_number, 2);
        assert_eq!(response.matches[0].line_matches[0].match_start, 6);
        assert_eq!(response.matches[0].line_matches[0].match_end, 11);
    }
}
