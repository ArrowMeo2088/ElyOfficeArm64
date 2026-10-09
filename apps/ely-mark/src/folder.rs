//! An opened folder's Markdown files, and searching them (MARK-05, MARK-10).

use std::{
    io,
    path::{Path, PathBuf},
};

/// An opened folder and its Markdown files.
pub struct Folder {
    pub root: PathBuf,
    pub files: Vec<String>,
}

/// Hits a search keeps; it looks no further.
pub const MOST_HITS: usize = 500;

fn markdown(name: &str) -> bool {
    let name = name.to_lowercase();
    name.ends_with(".md") || name.ends_with(".markdown")
}

/// Markdown files under `root`; no links, no dot names.
pub fn markdown_under(root: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    let mut folders = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = folders.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let kind = entry.file_type()?;
            let path = format!("{prefix}{name}");
            if kind.is_dir() {
                folders.push((entry.path(), format!("{path}/")));
            } else if kind.is_file() && markdown(&name) {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|path| path.to_lowercase());
    Ok(found)
}

/// One line that holds the words.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub file: PathBuf,
    /// Counted from one.
    pub line: usize,
    pub text: String,
}

/// Lines of `text` holding `query`, any case.
pub fn lines_with(file: &Path, text: &str, query: &str, hits: &mut Vec<Hit>) {
    let query = query.to_lowercase();
    for (at, line) in text.lines().enumerate() {
        if hits.len() == MOST_HITS {
            return;
        }
        if line.to_lowercase().contains(&query) {
            let shown: String = line.trim().chars().take(160).collect();
            hits.push(Hit {
                file: file.to_path_buf(),
                line: at + 1,
                text: shown,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[cfg(unix)]
    #[test]
    fn markdown_files_list_without_hidden_or_other_files() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("notes/deep")).expect("makes");
        std::fs::create_dir_all(root.join(".git")).expect("makes");
        for file in [
            "b.md",
            "A.markdown",
            "notes/deep/c.MD",
            "notes/skip.txt",
            ".git/x.md",
            ".hidden.md",
        ] {
            std::fs::write(root.join(file), "x").expect("writes");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("notes"), root.join("loop")).expect("links");
        let found = super::markdown_under(root).expect("lists");
        assert_eq!(found, ["A.markdown", "b.md", "notes/deep/c.MD"]);
    }

    #[test]
    fn lines_with_the_words_any_case() {
        let mut hits = Vec::new();
        super::lines_with(
            Path::new("a.md"),
            "# Title\nsome Words here\nnone\nWORDS",
            "words",
            &mut hits,
        );
        let lines: Vec<_> = hits
            .iter()
            .map(|hit| (hit.line, hit.text.as_str()))
            .collect();
        assert_eq!(lines, [(2, "some Words here"), (4, "WORDS")]);
    }
}
