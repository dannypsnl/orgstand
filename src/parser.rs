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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_heading_level_star() {
        assert_eq!(heading_level("* foo"), Some(1));
    }

    #[test]
    fn test_heading_level_multi_star() {
        assert_eq!(heading_level("*** bar"), Some(3));
    }

    #[test]
    fn test_heading_level_not_heading() {
        assert_eq!(heading_level("normal text"), None);
        assert_eq!(heading_level(""), None);
    }

    #[test]
    fn test_extract_entry_content_single() {
        let lines = vec!["* TODO Task", "some body", "more body"];
        let content = extract_entry_content(&lines, 0, 1);
        assert!(content.contains("* TODO Task"));
        assert!(content.contains("some body"));
        assert!(content.contains("more body"));
    }

    #[test]
    fn test_extract_entry_content_stops_at_sibling() {
        let lines = vec!["* TODO First", "body", "* TODO Second"];
        let content = extract_entry_content(&lines, 0, 1);
        assert!(content.contains("* TODO First"));
        assert!(content.contains("body"));
        assert!(!content.contains("* TODO Second"));
    }

    #[test]
    fn test_extract_entry_content_includes_children() {
        let lines = vec!["* Parent", "** Child", "child body", "* Sibling"];
        let content = extract_entry_content(&lines, 0, 1);
        assert!(content.contains("** Child"));
        assert!(content.contains("child body"));
        assert!(!content.contains("* Sibling"));
    }

    #[test]
    fn test_extract_todos_from_content_keyword() {
        let content = "* TODO Fix bug\nsome description\n* DONE Old task\n";
        let path = Path::new("test.org");
        let todos = extract_todos_from_content(content, path);
        assert_eq!(todos.len(), 2);

        let todo = todos.iter().find(|t| t.keyword == "TODO").unwrap();
        assert_eq!(todo.title, "Fix bug");

        let done = todos.iter().find(|t| t.keyword == "DONE").unwrap();
        assert_eq!(done.title, "Old task");
    }

    #[test]
    fn test_extract_todos_from_content_no_keyword() {
        let content = "* A plain note\nbody text\n";
        let path = Path::new("test.org");
        let todos = extract_todos_from_content(content, path);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].keyword, "");
        assert_eq!(todos[0].title, "A plain note");
    }

    #[test]
    fn test_extract_todos_tags() {
        let content = "* TODO Task with tags                                             :work:home:\n";
        let path = Path::new("test.org");
        let todos = extract_todos_from_content(content, path);
        assert_eq!(todos.len(), 1);
        assert!(todos[0].tags.contains(&"work".to_string()));
        assert!(todos[0].tags.contains(&"home".to_string()));
    }
}
