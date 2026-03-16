use anyhow::Result;
use std::path::Path;

use crate::scanner::scan_org_files;
use crate::types::TodoEntry;

pub fn heading_level(line: &str) -> Option<usize> {
    if line.starts_with('*') {
        Some(line.chars().take_while(|c| *c == '*').count())
    } else {
        None
    }
}

pub fn extract_entry_content(lines: &[&str], start: usize, level: usize) -> String {
    let mut content = String::new();
    content.push_str(lines[start]);
    content.push('\n');

    for line in lines.iter().skip(start + 1) {
        if let Some(next_level) = heading_level(line) {
            if next_level <= level {
                break;
            }
        }
        content.push_str(line);
        content.push('\n');
    }

    content
}

pub fn extract_todos_from_content(content: &str, file_path: &Path) -> Vec<TodoEntry> {
    use orgize::Org;

    let mut todos = Vec::new();
    let org = Org::parse(content);
    let lines: Vec<&str> = content.lines().collect();

    for headline in org.headlines() {
        let title_obj = headline.title(&org);
        let level = headline.level();

        let keyword = title_obj
            .keyword
            .as_ref()
            .map(|k| k.to_string())
            .unwrap_or_default();
        let title = title_obj.raw.trim().to_string();
        let tags: Vec<String> = title_obj.tags.iter().map(|t| t.to_string()).collect();

        let stars = "*".repeat(level);
        if let Some(i) = lines.iter().position(|line| {
            line.starts_with(&stars)
                && !line.get(level..).map_or(false, |s| s.starts_with('*'))
                && line.contains(title_obj.raw.trim())
        }) {
            let entry_content = extract_entry_content(&lines, i, level);
            todos.push(TodoEntry {
                keyword,
                title,
                tags,
                file_path: file_path.to_path_buf(),
                content: entry_content,
                level,
            });
        }
    }

    todos
}

pub fn extract_all_todos(dir: &Path) -> Result<Vec<TodoEntry>> {
    let files = scan_org_files(dir)?;
    let mut all_todos = Vec::new();

    for file_path in files {
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            let todos = extract_todos_from_content(&content, &file_path);
            all_todos.extend(todos);
        }
    }

    Ok(all_todos)
}
