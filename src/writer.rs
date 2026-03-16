use anyhow::Result;
use chrono::Local;
use std::path::Path;

use crate::parser::heading_level;
use crate::types::{DateInputType, TodoEntry};

pub fn skip_entry_content(lines: &[&str], start: usize, level: usize) -> usize {
    let mut i = start;
    while i < lines.len() {
        if let Some(next_level) = heading_level(lines[i]) {
            if next_level <= level {
                break;
            }
        }
        i += 1;
    }
    i
}

pub fn update_todo_in_file(file_path: &Path, todo: &TodoEntry, new_content: &str) -> Result<()> {
    let original_content = std::fs::read_to_string(file_path)?;
    let lines: Vec<&str> = original_content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut found = false;

    while i < lines.len() {
        let line = lines[i];

        if let Some(level) = heading_level(line) {
            if line.contains(&todo.keyword) && line.contains(&todo.title) {
                result.push(new_content);
                found = true;
                i = skip_entry_content(&lines, i + 1, level);
                continue;
            }
        }

        result.push(line);
        i += 1;
    }

    if !found {
        return Err(anyhow::anyhow!("TODO entry not found in file"));
    }

    std::fs::write(file_path, result.join("\n"))?;
    Ok(())
}

pub fn add_date_to_file(todo: &TodoEntry, date: &str, input_type: &DateInputType) -> Result<()> {
    let (new_line, keyword_to_detect) = match input_type {
        DateInputType::Scheduled => (format!("SCHEDULED: <{}>", date), Some("SCHEDULED:")),
        DateInputType::Deadline => (format!("DEADLINE: <{}>", date), Some("DEADLINE:")),
        DateInputType::Plain => (format!("<{}>", date), None),
    };

    let file_content = std::fs::read_to_string(&todo.file_path)?;
    let lines: Vec<&str> = file_content.lines().collect();
    let mut result = Vec::new();
    let mut found = false;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if !found && line.starts_with('*') && line.contains(&todo.title) {
            result.push(line.to_string());

            if let Some(next) = lines.get(i + 1) {
                let should_skip = match keyword_to_detect {
                    Some(kw) => next.contains(kw),
                    None => {
                        let trimmed = next.trim();
                        trimmed.starts_with('<')
                            && trimmed.ends_with('>')
                            && !next.contains("SCHEDULED:")
                            && !next.contains("DEADLINE:")
                    }
                };
                if should_skip {
                    i += 1;
                }
            }
            result.push(new_line.clone());
            found = true;
        } else {
            result.push(line.to_string());
        }
        i += 1;
    }

    std::fs::write(&todo.file_path, result.join("\n"))?;
    Ok(())
}

pub fn toggle_keyword_in_file(todo: &TodoEntry, new_keyword: &str) -> Result<()> {
    let file_content = std::fs::read_to_string(&todo.file_path)?;
    let updated_content = file_content.replace(
        &format!("* {} {}", todo.keyword, todo.title),
        &format!("* {} {}", new_keyword, todo.title),
    );
    std::fs::write(&todo.file_path, updated_content)?;
    Ok(())
}

pub fn delete_entry_from_file(todo: &TodoEntry) -> Result<()> {
    let file_content = std::fs::read_to_string(&todo.file_path)?;
    let lines: Vec<&str> = file_content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut found = false;

    while i < lines.len() {
        let line = lines[i];

        if !found {
            if let Some(level) = heading_level(line) {
                if line.contains(&todo.keyword) && line.contains(&todo.title) {
                    found = true;
                    i = skip_entry_content(&lines, i + 1, level);
                    continue;
                }
            }
        }

        result.push(line);
        i += 1;
    }

    std::fs::write(&todo.file_path, result.join("\n"))?;
    Ok(())
}

pub fn update_tags_in_file(todo: &TodoEntry, new_tags: &[String]) -> Result<()> {
    let file_content = std::fs::read_to_string(&todo.file_path)?;
    let lines: Vec<&str> = file_content.lines().collect();
    let mut result = Vec::new();

    for line in lines.iter() {
        if line.starts_with('*') && line.contains(&todo.keyword) && line.contains(&todo.title) {
            let stars = "*".repeat(todo.level);
            let base = format!("{} {} {}", stars, todo.keyword, todo.title);

            let new_line = if !new_tags.is_empty() {
                let tags_str = format!(":{}:", new_tags.join(":"));
                let target_col = 77;
                let current_len = base.len();
                let tags_len = tags_str.len();

                let padding = if current_len + 1 + tags_len <= target_col {
                    target_col - current_len - tags_len
                } else {
                    1
                };

                format!("{}{}{}", base, " ".repeat(padding), tags_str)
            } else {
                base
            };

            result.push(new_line);
        } else {
            result.push(line.to_string());
        }
    }

    std::fs::write(&todo.file_path, result.join("\n"))?;
    Ok(())
}

pub fn append_new_note(directory: &Path) -> Result<()> {
    let inbox_path = directory.join("inbox.org");
    let now = Local::now();
    let new_entry = format!(
        "\n* New Note\n:PROPERTIES:\n:CREATED: [{}]\n:END:\n",
        now.format("%Y-%m-%d %a %H:%M:%S")
    );

    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&inbox_path)?;
    file.write_all(new_entry.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_todo(title: &str, keyword: &str, level: usize, file_path: &PathBuf) -> TodoEntry {
        let content = if keyword.is_empty() {
            format!("{} {}\n", "*".repeat(level), title)
        } else {
            format!("{} {} {}\n", "*".repeat(level), keyword, title)
        };
        TodoEntry {
            keyword: keyword.to_string(),
            title: title.to_string(),
            tags: vec![],
            file_path: file_path.clone(),
            content,
            level,
        }
    }

    #[test]
    fn test_skip_entry_content_stops_at_sibling() {
        let lines = vec!["body1", "body2", "* Next heading", "body3"];
        let end = skip_entry_content(&lines, 0, 1);
        assert_eq!(end, 2);
    }

    #[test]
    fn test_skip_entry_content_skips_children() {
        let lines = vec!["** child", "body", "* next"];
        let end = skip_entry_content(&lines, 0, 1);
        assert_eq!(end, 2);
    }

    #[test]
    fn test_toggle_keyword_in_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.org");
        std::fs::write(&file_path, "* TODO Fix bug\nbody\n").unwrap();
        let todo = make_todo("Fix bug", "TODO", 1, &file_path);

        toggle_keyword_in_file(&todo, "DONE").unwrap();
        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(result.contains("* DONE Fix bug"));
        assert!(!result.contains("* TODO Fix bug"));
    }

    #[test]
    fn test_update_todo_in_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.org");
        std::fs::write(&file_path, "* TODO Fix bug\nold body\n* TODO Other\n").unwrap();
        let todo = make_todo("Fix bug", "TODO", 1, &file_path);

        update_todo_in_file(&file_path, &todo, "* TODO Fix bug\nnew body\n").unwrap();
        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(result.contains("new body"));
        assert!(!result.contains("old body"));
        assert!(result.contains("* TODO Other"));
    }

    #[test]
    fn test_delete_entry_from_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.org");
        std::fs::write(&file_path, "* TODO Remove me\nbody\n* TODO Keep me\n").unwrap();
        let todo = make_todo("Remove me", "TODO", 1, &file_path);

        delete_entry_from_file(&todo).unwrap();
        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(!result.contains("Remove me"));
        assert!(result.contains("Keep me"));
    }

    #[test]
    fn test_add_date_to_file_scheduled() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.org");
        std::fs::write(&file_path, "* TODO Task\nbody\n").unwrap();
        let todo = make_todo("Task", "TODO", 1, &file_path);

        add_date_to_file(&todo, "2026-03-16 Mon 09:00", &DateInputType::Scheduled).unwrap();
        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(result.contains("SCHEDULED: <2026-03-16 Mon 09:00>"));
    }

    #[test]
    fn test_update_tags_in_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.org");
        std::fs::write(&file_path, "* TODO Task\nbody\n").unwrap();
        let todo = make_todo("Task", "TODO", 1, &file_path);

        update_tags_in_file(&todo, &["work".to_string(), "urgent".to_string()]).unwrap();
        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(result.contains(":work:urgent:"));
    }

    #[test]
    fn test_update_tags_in_file_empty_tags() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.org");
        std::fs::write(&file_path, "* TODO Task                                              :old:\nbody\n").unwrap();
        let todo = make_todo("Task", "TODO", 1, &file_path);

        update_tags_in_file(&todo, &[]).unwrap();
        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(!result.contains(":old:"));
    }
}
pub fn append_quick_capture(directory: &Path, title: &str) -> Result<()> {
    // NOTE: directory must exist; inbox.org is created if absent
    let inbox_path = directory.join("inbox.org");
    let now = Local::now();
    let today = now.date_naive();

    let new_entry = format!(
        "\n* TODO {}\nSCHEDULED: <{}>\n:PROPERTIES:\n:CREATED: [{}]\n:END:\n",
        title,
        today.format("%Y-%m-%d %a"),
        now.format("%Y-%m-%d %a %H:%M:%S")
    );

    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&inbox_path)?;
    file.write_all(new_entry.as_bytes())?;
    Ok(())
}
