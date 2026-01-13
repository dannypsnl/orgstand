use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use std::io;
use std::path::{Path, PathBuf};
use tui_textarea::TextArea;

#[derive(Clone)]
struct TodoEntry {
    keyword: String,      // TODO, DONE, etc.
    title: String,        // The title text (without tags)
    tags: Vec<String>,    // Tags from the heading
    file_path: PathBuf,   // Which file it's in
    content: String,      // Full content of this entry (including children)
    level: usize,
}

#[derive(Clone, PartialEq)]
enum ViewFilter {
    All,
    Today,
}

#[derive(Clone)]
enum DateInputType {
    Scheduled,
    Deadline,
}

enum Mode {
    Browser {
        todos: Vec<TodoEntry>,
        selected: usize,
        filter: ViewFilter,
    },
    Viewer {
        todo: TodoEntry,
        scroll: u16,
    },
    Editor {
        todo: TodoEntry,
        textarea: TextArea<'static>,
    },
    DateInput {
        todo: TodoEntry,
        input_type: DateInputType,
        selected_date: NaiveDate,
        viewing_month: NaiveDate, // First day of the month being viewed
        hour: u32,
        minute: u32,
        editing_time: bool, // true if currently editing time, false if editing date
    },
    TagManagement {
        todo: TodoEntry,
        tag_input: TextArea<'static>,
    },
    QuickCapture {
        title_input: TextArea<'static>,
    },
}

struct App {
    mode: Mode,
    directory: PathBuf,
    last_filter: ViewFilter,
}

impl App {
    fn new(path_arg: Option<String>) -> Result<Self> {
        let directory = if let Some(path_str) = path_arg {
            let path = PathBuf::from(&path_str);
            if path.is_dir() {
                path
            } else {
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            }
        } else {
            std::env::current_dir()?
        };

        let todos = Self::extract_all_todos(&directory)?;

        Ok(App {
            mode: Mode::Browser {
                todos,
                selected: 0,
                filter: ViewFilter::Today, // Default to agenda view (today's todos)
            },
            directory,
            last_filter: ViewFilter::Today,
        })
    }

    fn scan_org_files(dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        Self::scan_org_files_recursive(dir, &mut files, 0)?;
        files.sort();
        Ok(files)
    }

    fn scan_org_files_recursive(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
        // Limit recursion depth to avoid scanning too deep
        const MAX_DEPTH: usize = 5;
        if depth > MAX_DEPTH {
            return Ok(());
        }

        // Skip directories that should be ignored
        if let Some(dir_name) = dir.file_name().and_then(|n| n.to_str()) {
            // Skip hidden directories (starting with .)
            if dir_name.starts_with('.') {
                return Ok(());
            }

            // Skip common large directories
            const SKIP_DIRS: &[&str] = &[
                "node_modules",
                "target",
                "build",
                "dist",
                ".git",
                ".svn",
                "__pycache__",
                "venv",
                "env",
                "Library",
                "Applications",
                "System",
            ];

            if SKIP_DIRS.contains(&dir_name) {
                return Ok(());
            }
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("org") {
                    files.push(path);
                } else if path.is_dir() {
                    // Recursively scan subdirectories
                    Self::scan_org_files_recursive(&path, files, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn extract_all_todos(dir: &Path) -> Result<Vec<TodoEntry>> {
        let files = Self::scan_org_files(dir)?;
        let mut all_todos = Vec::new();

        for file_path in files {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let todos = Self::extract_todos_from_content(&content, &file_path);
                all_todos.extend(todos);
            }
        }

        Ok(all_todos)
    }

    fn extract_todos_from_content(content: &str, file_path: &Path) -> Vec<TodoEntry> {
        use orgize::Org;

        let mut todos = Vec::new();
        let org = Org::parse(content);

        // Use orgize to parse headlines
        for headline in org.headlines() {
            let title_obj = headline.title(&org);
            let title_raw = title_obj.raw.to_string();

            // Check if it has a TODO keyword
            // In orgize, we need to check the original line for the keyword
            // The title.raw contains just the title without keyword
            let lines: Vec<&str> = content.lines().collect();

            // Find the line that starts with the right number of stars and contains our title
            let level = headline.level();
            let stars = "*".repeat(level);

            for (i, line) in lines.iter().enumerate() {
                if line.starts_with(&stars) && line.contains(title_raw.as_str()) {
                    // Parse the heading line
                    let parts: Vec<&str> = line.splitn(3, ' ').collect();
                    if parts.len() >= 2 {
                        let potential_keyword = parts[1];

                        let (keyword, title, tags) = if matches!(
                            potential_keyword,
                            "TODO" | "DONE" | "NEXT" | "WAITING" | "CANCELLED" | "CANCELED"
                        ) {
                            // It has a TODO keyword
                            let keyword = potential_keyword.to_string();
                            let rest = parts.get(2).unwrap_or(&"");
                            let (title, tags) = Self::parse_title_and_tags(rest);
                            (keyword, title, tags)
                        } else {
                            // It's a Note without a keyword
                            // The title starts from parts[1] (after the stars)
                            let rest = &line[stars.len()..].trim_start();
                            let (title, tags) = Self::parse_title_and_tags(rest);
                            (String::new(), title, tags)
                        };

                        // Extract content until next heading of same or higher level
                        let mut entry_content = String::new();
                        entry_content.push_str(line);
                        entry_content.push('\n');

                        let mut j = i + 1;
                        while j < lines.len() {
                            let next_line = lines[j];
                            if next_line.starts_with('*') {
                                let next_level = next_line.chars().take_while(|c| *c == '*').count();
                                if next_level <= level {
                                    break;
                                }
                            }
                            entry_content.push_str(next_line);
                            entry_content.push('\n');
                            j += 1;
                        }

                        todos.push(TodoEntry {
                            keyword,
                            title,
                            tags,
                            file_path: file_path.to_path_buf(),
                            content: entry_content,
                            level,
                        });
                        break;
                    }
                }
            }
        }

        todos
    }

    fn parse_title_and_tags(text: &str) -> (String, Vec<String>) {
        // Tags are at the end in format :tag1:tag2:tag3:
        // They are right-aligned with spaces before them
        let text = text.trim();

        // Split from the right to get the last whitespace-separated token
        // This handles the right-aligned tags with padding spaces
        let parts: Vec<&str> = text.rsplitn(2, char::is_whitespace).collect();

        if parts.len() == 2 {
            let potential_tags = parts[0].trim();
            let potential_title = parts[1].trim();

            // Check if this looks like tags: starts with : ends with : and has at least one more :
            if potential_tags.starts_with(':')
                && potential_tags.ends_with(':')
                && potential_tags.matches(':').count() >= 2 {
                // Parse the tags
                let tags: Vec<String> = potential_tags
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                return (potential_title.to_string(), tags);
            }
        }

        // No valid tags found
        (text.to_string(), Vec::new())
    }

    fn open_todo(&mut self) -> Result<()> {
        if let Mode::Browser { todos, selected, filter } = &self.mode {
            self.last_filter = filter.clone();
            let filtered_todos = Self::filter_todos(todos, filter);
            if let Some(todo) = filtered_todos.get(*selected) {
                self.mode = Mode::Viewer {
                    todo: todo.clone(),
                    scroll: 0,
                };
            }
        }
        Ok(())
    }

    fn enter_edit_mode(&mut self) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let mut textarea = TextArea::new(todo.content.lines().map(|s| s.to_string()).collect());
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Editing: [{}] {}", todo.keyword, todo.title)),
            );
            self.mode = Mode::Editor {
                todo: todo.clone(),
                textarea,
            };
        }
        Ok(())
    }

    fn save_todo(&mut self) -> Result<()> {
        if let Mode::Editor { todo, textarea } = &self.mode {
            let new_content = textarea.lines().join("\n");
            Self::update_todo_in_file(&todo.file_path, todo, &new_content)?;

            // Return to browser and refresh
            let todos = Self::extract_all_todos(&self.directory)?;
            self.mode = Mode::Browser {
                todos,
                selected: 0,
                filter: self.last_filter.clone(),
            };
        }
        Ok(())
    }

    fn update_todo_in_file(file_path: &Path, todo: &TodoEntry, new_content: &str) -> Result<()> {
        let original_content = std::fs::read_to_string(file_path)?;
        let lines: Vec<&str> = original_content.lines().collect();
        let mut result = Vec::new();
        let mut i = 0;
        let mut found = false;

        while i < lines.len() {
            let line = lines[i];

            // Look for the matching TODO entry
            if line.starts_with('*') && line.contains(&todo.keyword) && line.contains(&todo.title) {
                // Found our TODO, replace its content
                result.push(new_content);
                found = true;

                // Skip the old content (until next heading of same or higher level)
                let level = line.chars().take_while(|c| *c == '*').count();
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i];
                    if next_line.starts_with('*') {
                        let next_level = next_line.chars().take_while(|c| *c == '*').count();
                        if next_level <= level {
                            break;
                        }
                    }
                    i += 1;
                }
                continue;
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

    fn back_to_browser(&mut self) -> Result<()> {
        let todos = Self::extract_all_todos(&self.directory)?;
        self.mode = Mode::Browser {
            todos,
            selected: 0,
            filter: self.last_filter.clone(),
        };
        Ok(())
    }

    fn filter_todos(todos: &[TodoEntry], filter: &ViewFilter) -> Vec<TodoEntry> {
        let mut filtered: Vec<TodoEntry> = match filter {
            ViewFilter::All => {
                // In All view, filter out DONE items
                // But keep Notes (items without keywords)
                todos
                    .iter()
                    .filter(|todo| todo.keyword != "DONE")
                    .cloned()
                    .collect()
            }
            ViewFilter::Today => {
                let today = Local::now().date_naive();
                todos
                    .iter()
                    .filter(|todo| {
                        // First check for SCHEDULED date
                        if let Some(scheduled_date) =
                            Self::parse_existing_date(&todo.content, &DateInputType::Scheduled)
                        {
                            return scheduled_date == today;
                        }

                        // If no SCHEDULED date, check for any date in the content
                        if let Some(any_date) = Self::parse_any_date(&todo.content) {
                            return any_date == today;
                        }

                        false
                    })
                    .cloned()
                    .collect()
            }
        };

        // Sort by date (earliest first)
        // First try SCHEDULED date, then fall back to any date
        filtered.sort_by_key(|todo| {
            Self::parse_existing_date(&todo.content, &DateInputType::Scheduled)
                .or_else(|| Self::parse_any_date(&todo.content))
        });

        filtered
    }

    fn toggle_view_filter(&mut self) {
        if let Mode::Browser { todos, selected, filter } = &self.mode {
            let new_filter = match filter {
                ViewFilter::All => ViewFilter::Today,
                ViewFilter::Today => ViewFilter::All,
            };
            self.last_filter = new_filter.clone();
            self.mode = Mode::Browser {
                todos: todos.clone(),
                selected: 0, // Reset selection when changing filter
                filter: new_filter,
            };
        }
    }

    fn toggle_todo_state(&mut self) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let new_keyword = if todo.keyword == "TODO" {
                "DONE"
            } else {
                "TODO"
            };

            // Update the keyword in the file
            let file_content = std::fs::read_to_string(&todo.file_path)?;
            let updated_content = file_content.replace(
                &format!("* {} {}", todo.keyword, todo.title),
                &format!("* {} {}", new_keyword, todo.title),
            );
            std::fs::write(&todo.file_path, updated_content)?;

            // Reload the todo
            let todos = Self::extract_all_todos(&self.directory)?;
            if let Some(updated) = todos.iter().find(|t| t.title == todo.title && t.file_path == todo.file_path) {
                self.mode = Mode::Viewer {
                    todo: updated.clone(),
                    scroll: 0,
                };
            }
        }
        Ok(())
    }

    fn enter_date_input(&mut self, input_type: DateInputType) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let today = Local::now().date_naive();

            // Try to parse existing date and time from todo content
            let initial_date = Self::parse_existing_date(&todo.content, &input_type).unwrap_or(today);
            let (hour, minute) = Self::parse_existing_time(&todo.content, &input_type);

            self.mode = Mode::DateInput {
                todo: todo.clone(),
                input_type,
                selected_date: initial_date,
                viewing_month: NaiveDate::from_ymd_opt(initial_date.year(), initial_date.month(), 1).unwrap(),
                hour,
                minute,
                editing_time: false,
            };
        }
        Ok(())
    }

    fn parse_existing_date(content: &str, input_type: &DateInputType) -> Option<NaiveDate> {
        let keyword = match input_type {
            DateInputType::Scheduled => "SCHEDULED:",
            DateInputType::Deadline => "DEADLINE:",
        };

        // Look for lines containing the keyword
        for line in content.lines() {
            if let Some(pos) = line.find(keyword) {
                // Extract date from <YYYY-MM-DD ...>
                let rest = &line[pos + keyword.len()..];
                if let Some(start) = rest.find('<') {
                    if let Some(end) = rest.find('>') {
                        let date_str = &rest[start + 1..end];
                        // Parse YYYY-MM-DD (ignore time and day of week)
                        let parts: Vec<&str> = date_str.split_whitespace().collect();
                        if let Some(date_part) = parts.first() {
                            if let Ok(date) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                                return Some(date);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn parse_any_date(content: &str) -> Option<NaiveDate> {
        // Parse any date in <YYYY-MM-DD ...> format, regardless of whether it has SCHEDULED: or DEADLINE:
        for line in content.lines() {
            let mut search_pos = 0;
            while let Some(start_pos) = line[search_pos..].find('<') {
                let actual_start = search_pos + start_pos;
                if let Some(end_pos) = line[actual_start..].find('>') {
                    let actual_end = actual_start + end_pos;
                    let date_str = &line[actual_start + 1..actual_end];
                    // Parse YYYY-MM-DD (ignore time and day of week)
                    let parts: Vec<&str> = date_str.split_whitespace().collect();
                    if let Some(date_part) = parts.first() {
                        if let Ok(date) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                            return Some(date);
                        }
                    }
                    search_pos = actual_end + 1;
                } else {
                    break;
                }
            }
        }
        None
    }

    fn parse_existing_time(content: &str, input_type: &DateInputType) -> (u32, u32) {
        let keyword = match input_type {
            DateInputType::Scheduled => "SCHEDULED:",
            DateInputType::Deadline => "DEADLINE:",
        };

        // Look for lines containing the keyword
        for line in content.lines() {
            if let Some(pos) = line.find(keyword) {
                // Extract time from <YYYY-MM-DD Day HH:MM>
                let rest = &line[pos + keyword.len()..];
                if let Some(start) = rest.find('<') {
                    if let Some(end) = rest.find('>') {
                        let date_str = &rest[start + 1..end];
                        let parts: Vec<&str> = date_str.split_whitespace().collect();
                        // Time is the third part (after date and day of week)
                        if parts.len() >= 3 {
                            if let Some(time_part) = parts.get(2) {
                                let time_components: Vec<&str> = time_part.split(':').collect();
                                if time_components.len() == 2 {
                                    if let (Ok(h), Ok(m)) = (
                                        time_components[0].parse::<u32>(),
                                        time_components[1].parse::<u32>(),
                                    ) {
                                        return (h, m);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Default to 00:00 if no time found
        (0, 0)
    }

    fn submit_date_input(&mut self) -> Result<()> {
        // Clone data first to avoid borrow conflicts
        let (todo_clone, input_type_clone, selected_date, hour, minute) = if let Mode::DateInput { todo, input_type, selected_date, hour, minute, .. } = &self.mode {
            (todo.clone(), input_type.clone(), *selected_date, *hour, *minute)
        } else {
            return Ok(());
        };

        // Format date as org-mode date string with day of week and time
        let weekday = selected_date.format("%a");
        let date_str = format!("{} {} {:02}:{:02}", selected_date.format("%Y-%m-%d"), weekday, hour, minute);

        // Temporarily switch to viewer mode to release borrow
        self.mode = Mode::Viewer {
            todo: todo_clone,
            scroll: 0,
        };

        match input_type_clone {
            DateInputType::Scheduled => {
                self.add_scheduled_date(&date_str)?;
            }
            DateInputType::Deadline => {
                self.add_deadline_date(&date_str)?;
            }
        }

        Ok(())
    }

    fn calendar_move_day(&mut self, days: i64) {
        if let Mode::DateInput { selected_date, viewing_month, editing_time, .. } = &mut self.mode {
            if !*editing_time {
                if let Some(new_date) = selected_date.checked_add_signed(chrono::Duration::days(days)) {
                    *selected_date = new_date;

                    // Update viewing month if we moved to a different month
                    if selected_date.year() != viewing_month.year() || selected_date.month() != viewing_month.month() {
                        *viewing_month = NaiveDate::from_ymd_opt(
                            selected_date.year(),
                            selected_date.month(),
                            1
                        ).unwrap();
                    }
                }
            }
        }
    }

    fn adjust_time(&mut self, hours_delta: i32, minutes_delta: i32) {
        if let Mode::DateInput { hour, minute, editing_time, .. } = &mut self.mode {
            if *editing_time {
                *hour = ((*hour as i32 + hours_delta).rem_euclid(24)) as u32;
                *minute = ((*minute as i32 + minutes_delta).rem_euclid(60)) as u32;
            }
        }
    }

    fn toggle_time_edit(&mut self) {
        if let Mode::DateInput { editing_time, .. } = &mut self.mode {
            *editing_time = !*editing_time;
        }
    }

    fn calendar_change_month(&mut self, months: i32) {
        if let Mode::DateInput { selected_date, viewing_month, .. } = &mut self.mode {
            let new_year = viewing_month.year();
            let new_month = viewing_month.month() as i32 + months;

            let (final_year, final_month) = if new_month <= 0 {
                (new_year - 1, (12 + new_month) as u32)
            } else if new_month > 12 {
                (new_year + 1, (new_month - 12) as u32)
            } else {
                (new_year, new_month as u32)
            };

            if let Some(new_viewing) = NaiveDate::from_ymd_opt(final_year, final_month, 1) {
                *viewing_month = new_viewing;

                // Adjust selected date to be in the new month if it's outside
                if selected_date.year() != final_year || selected_date.month() != final_month {
                    *selected_date = new_viewing;
                }
            }
        }
    }

    fn render_calendar(viewing_month: NaiveDate, selected_date: NaiveDate) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Month and year header
        let header = format!("{}", viewing_month.format("%B %Y"));
        lines.push(Line::from(vec![
            ratatui::text::Span::styled(header, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]));
        lines.push(Line::from(""));

        // Weekday headers
        lines.push(Line::from("Su  Mo  Tu  We  Th  Fr  Sa"));
        lines.push(Line::from("───────────────────────────"));

        // Get first day of month and its weekday
        let first_day = NaiveDate::from_ymd_opt(viewing_month.year(), viewing_month.month(), 1).unwrap();
        let first_weekday = first_day.weekday().num_days_from_sunday();

        // Get last day of month
        let next_month = if viewing_month.month() == 12 {
            NaiveDate::from_ymd_opt(viewing_month.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(viewing_month.year(), viewing_month.month() + 1, 1).unwrap()
        };
        let last_day = next_month.pred_opt().unwrap().day();

        // Build calendar grid
        let mut week_line = String::new();
        // Add spacing for days before first of month
        for _ in 0..first_weekday {
            week_line.push_str("    ");
        }

        for day in 1..=last_day {
            let current_date = NaiveDate::from_ymd_opt(viewing_month.year(), viewing_month.month(), day).unwrap();
            let day_str = format!("{:2}", day);

            if current_date == selected_date {
                // Selected date - highlighted
                week_line.push_str(&format!("[{}]", day_str));
            } else if current_date == Local::now().date_naive() {
                // Today - marked with *
                week_line.push_str(&format!(" {}*", day_str));
            } else {
                week_line.push_str(&format!(" {} ", day_str));
            }

            let current_weekday = (first_weekday + day - 1) % 7;
            if current_weekday == 6 {
                lines.push(Line::from(week_line.clone()));
                week_line.clear();
            }
        }

        if !week_line.is_empty() {
            lines.push(Line::from(week_line));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(format!("Selected: {}", selected_date.format("%Y-%m-%d %a"))));

        lines
    }

    fn cancel_date_input(&mut self) -> Result<()> {
        if let Mode::DateInput { todo, .. } = &self.mode {
            self.mode = Mode::Viewer {
                todo: todo.clone(),
                scroll: 0,
            };
        }
        Ok(())
    }

    fn add_scheduled_date(&mut self, date: &str) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let file_content = std::fs::read_to_string(&todo.file_path)?;
            let lines: Vec<&str> = file_content.lines().collect();
            let mut result = Vec::new();
            let mut found = false;

            let scheduled_line = format!("SCHEDULED: <{}>", date);

            let mut i = 0;
            while i < lines.len() {
                let line = lines[i];

                // Find the TODO line and add SCHEDULED after it
                if !found && line.starts_with('*') && line.contains(&todo.keyword) && line.contains(&todo.title) {
                    result.push(line);

                    // Check if next line already has SCHEDULED
                    let next_line = lines.get(i + 1).unwrap_or(&"");
                    if next_line.contains("SCHEDULED:") {
                        // Skip the old SCHEDULED line by incrementing i
                        i += 1;
                    }
                    result.push(&scheduled_line);
                    found = true;
                } else {
                    result.push(line);
                }

                i += 1;
            }

            std::fs::write(&todo.file_path, result.join("\n"))?;

            // Return to browser
            self.back_to_browser()?;
        }
        Ok(())
    }

    fn add_deadline_date(&mut self, date: &str) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let file_content = std::fs::read_to_string(&todo.file_path)?;
            let lines: Vec<&str> = file_content.lines().collect();
            let mut result = Vec::new();
            let mut found = false;

            let deadline_line = format!("DEADLINE: <{}>", date);

            let mut i = 0;
            while i < lines.len() {
                let line = lines[i];

                // Find the TODO line and add DEADLINE after it
                if !found && line.starts_with('*') && line.contains(&todo.keyword) && line.contains(&todo.title) {
                    result.push(line);

                    // Check if next line already has DEADLINE
                    let next_line = lines.get(i + 1).unwrap_or(&"");
                    if next_line.contains("DEADLINE:") {
                        // Skip the old DEADLINE line by incrementing i
                        i += 1;
                    }
                    result.push(&deadline_line);
                    found = true;
                } else {
                    result.push(line);
                }

                i += 1;
            }

            std::fs::write(&todo.file_path, result.join("\n"))?;

            // Return to browser
            self.back_to_browser()?;
        }
        Ok(())
    }

    fn exit_edit_mode_with_save(&mut self) -> Result<()> {
        // Extract data from Editor mode first
        let (todo_clone, new_content) = if let Mode::Editor { todo, textarea } = &self.mode {
            let content = textarea.lines().join("\n");
            (todo.clone(), content)
        } else {
            return Ok(());
        };

        // Save to file
        Self::update_todo_in_file(&todo_clone.file_path, &todo_clone, &new_content)?;

        // Return to browser after editing
        self.back_to_browser()?;

        Ok(())
    }

    fn get_selected_todo_from_browser(&self) -> Option<TodoEntry> {
        if let Mode::Browser { todos, selected, filter } = &self.mode {
            let filtered_todos = Self::filter_todos(todos, filter);
            filtered_todos.get(*selected).cloned()
        } else {
            None
        }
    }

    fn toggle_todo_state_from_browser(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let new_keyword = if todo.keyword == "TODO" { "DONE" } else { "TODO" };

            // Update the keyword in the file
            let file_content = std::fs::read_to_string(&todo.file_path)?;
            let updated_content = file_content.replace(
                &format!("* {} {}", todo.keyword, todo.title),
                &format!("* {} {}", new_keyword, todo.title),
            );
            std::fs::write(&todo.file_path, updated_content)?;

            // Refresh browser
            self.back_to_browser()?;
        }
        Ok(())
    }

    fn enter_date_input_from_browser(&mut self, input_type: DateInputType) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let today = Local::now().date_naive();
            let initial_date = Self::parse_existing_date(&todo.content, &input_type).unwrap_or(today);
            let (hour, minute) = Self::parse_existing_time(&todo.content, &input_type);

            self.mode = Mode::DateInput {
                todo,
                input_type,
                selected_date: initial_date,
                viewing_month: NaiveDate::from_ymd_opt(initial_date.year(), initial_date.month(), 1).unwrap(),
                hour,
                minute,
                editing_time: false,
            };
        }
        Ok(())
    }

    fn enter_edit_mode_from_browser(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let mut textarea = TextArea::new(todo.content.lines().map(|s| s.to_string()).collect());
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Editing: [{}] {}", todo.keyword, todo.title)),
            );
            self.mode = Mode::Editor { todo, textarea };
        }
        Ok(())
    }

    fn delete_todo_from_browser(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            // Read file content
            let file_content = std::fs::read_to_string(&todo.file_path)?;
            let lines: Vec<&str> = file_content.lines().collect();
            let mut result = Vec::new();
            let mut i = 0;
            let mut found = false;

            while i < lines.len() {
                let line = lines[i];

                // Look for the matching TODO entry
                if !found && line.starts_with('*') && line.contains(&todo.keyword) && line.contains(&todo.title) {
                    found = true;
                    let level = line.chars().take_while(|c| *c == '*').count();

                    // Skip this entry and all its children
                    i += 1;
                    while i < lines.len() {
                        let next_line = lines[i];
                        if next_line.starts_with('*') {
                            let next_level = next_line.chars().take_while(|c| *c == '*').count();
                            if next_level <= level {
                                break;
                            }
                        }
                        i += 1;
                    }
                    continue;
                }

                result.push(line);
                i += 1;
            }

            std::fs::write(&todo.file_path, result.join("\n"))?;

            // Refresh browser
            self.back_to_browser()?;
        }
        Ok(())
    }

    fn create_new_note(&mut self) -> Result<()> {
        // Create a simple note (not TODO) in inbox.org
        let inbox_path = self.directory.join("inbox.org");

        // Create a basic note entry with timestamp
        let now = Local::now();
        let new_entry = format!(
            "\n* New Note\n:PROPERTIES:\n:CREATED: [{}]\n:END:\n",
            now.format("%Y-%m-%d %a %H:%M:%S")
        );

        // Append to inbox.org (create if doesn't exist)
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inbox_path)?;
        file.write_all(new_entry.as_bytes())?;

        // Refresh browser
        self.back_to_browser()?;
        Ok(())
    }

    fn enter_quick_capture(&mut self) -> Result<()> {
        let mut title_input = TextArea::new(vec![String::new()]);
        title_input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Quick Capture - Enter title (will be scheduled for today)"),
        );
        self.mode = Mode::QuickCapture { title_input };
        Ok(())
    }

    fn save_quick_capture(&mut self) -> Result<()> {
        let title = if let Mode::QuickCapture { title_input } = &self.mode {
            title_input.lines().join(" ").trim().to_string()
        } else {
            return Ok(());
        };

        if title.is_empty() {
            // Don't create empty entries
            self.back_to_browser()?;
            return Ok(());
        }

        // Create a TODO entry scheduled for today in inbox.org
        let inbox_path = self.directory.join("inbox.org");
        let now = Local::now();
        let today = now.date_naive();

        let new_entry = format!(
            "\n* TODO {}\nSCHEDULED: <{}>\n:PROPERTIES:\n:CREATED: [{}]\n:END:\n",
            title,
            today.format("%Y-%m-%d %a"),
            now.format("%Y-%m-%d %a %H:%M:%S")
        );

        // Append to inbox.org
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inbox_path)?;
        file.write_all(new_entry.as_bytes())?;

        // Refresh browser
        self.back_to_browser()?;
        Ok(())
    }

    fn enter_tag_management(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let current_tags = todo.tags.join(" ");
            let mut tag_input = TextArea::new(vec![current_tags]);
            tag_input.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Manage Tags for: {}", todo.title)),
            );
            self.mode = Mode::TagManagement { todo, tag_input };
        }
        Ok(())
    }

    fn save_tags(&mut self) -> Result<()> {
        let (todo_clone, new_tags_str) = if let Mode::TagManagement { todo, tag_input } = &self.mode {
            let tags_str = tag_input.lines().join(" ");
            (todo.clone(), tags_str)
        } else {
            return Ok(());
        };

        // Parse new tags
        let new_tags: Vec<String> = new_tags_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Update tags in the file
        self.update_tags_in_file(&todo_clone, &new_tags)?;

        // Return to browser
        self.back_to_browser()?;
        Ok(())
    }

    fn update_tags_in_file(&self, todo: &TodoEntry, new_tags: &[String]) -> Result<()> {
        let file_content = std::fs::read_to_string(&todo.file_path)?;
        let lines: Vec<&str> = file_content.lines().collect();
        let mut result = Vec::new();

        for line in lines.iter() {
            // Find the matching TODO heading line
            if line.starts_with('*') && line.contains(&todo.keyword) && line.contains(&todo.title) {
                // Reconstruct the line with new tags
                let stars = "*".repeat(todo.level);

                // Format: * KEYWORD TITLE + padding + :tag1:tag2:
                // Tags should be right-aligned at column 77 (org-mode standard)
                let base = format!("{} {} {}", stars, todo.keyword, todo.title);

                let new_line = if !new_tags.is_empty() {
                    let tags_str = format!(":{}:", new_tags.join(":"));
                    let target_col = 77;
                    let current_len = base.len();
                    let tags_len = tags_str.len();

                    // Calculate padding needed
                    let padding = if current_len + 1 + tags_len <= target_col {
                        target_col - current_len - tags_len
                    } else {
                        1 // At least one space
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

}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path_arg = args.get(1).cloned();

    let mut app = App::new(path_arg)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(f.area());

            match &app.mode {
                Mode::Browser { todos, selected, filter } => {
                    // Apply filter to todos
                    let filtered_todos = App::filter_todos(todos, filter);

                    // TODO entries browser
                    let items: Vec<ListItem> = filtered_todos
                        .iter()
                        .enumerate()
                        .map(|(i, todo)| {
                            let keyword_style = if todo.keyword == "TODO" {
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                            } else if todo.keyword == "DONE" {
                                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            };

                            let tags_str = if !todo.tags.is_empty() {
                                format!(" :{}: ", todo.tags.join(":"))
                            } else {
                                String::new()
                            };

                            let display = format!(
                                "[{}] {}{}  - {}",
                                todo.keyword,
                                todo.title,
                                tags_str,
                                todo.file_path.file_name().unwrap().to_string_lossy()
                            );

                            let style = if i == *selected {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            };

                            ListItem::new(display).style(style)
                        })
                        .collect();

                    let view_mode = match filter {
                        ViewFilter::All => "All TODOs",
                        ViewFilter::Today => "Today's Agenda",
                    };

                    let list = List::new(items).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!("{} ({}/{}) - {}", view_mode, filtered_todos.len(), todos.len(), app.directory.display())),
                    );
                    f.render_widget(list, chunks[0]);

                    let status = Paragraph::new("↑/↓: Navigate | Enter: View | t: Toggle | s: Schedule | d: Deadline | e: Edit | g: Tags | c: Capture | n: Note | x: Delete | Tab: View | q: Quit")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Gray));
                    f.render_widget(status, chunks[1]);
                }
                Mode::Viewer { todo, scroll } => {
                    // TODO viewer (read-only)
                    let title_style = if todo.keyword == "TODO" {
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                    } else if todo.keyword == "DONE" {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    };

                    let content = Paragraph::new(todo.content.as_str())
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(format!("[{}] {}", todo.keyword, todo.title))
                                .title_style(title_style),
                        )
                        .wrap(Wrap { trim: false })
                        .scroll((*scroll, 0));
                    f.render_widget(content, chunks[0]);

                    let status_text = format!(
                        "t: Toggle TODO/DONE | s: Schedule | d: Deadline | e: Edit | Esc: Back | q: Quit"
                    );
                    let status = Paragraph::new(status_text)
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Gray));
                    f.render_widget(status, chunks[1]);
                }
                Mode::DateInput { input_type, selected_date, viewing_month, hour, minute, editing_time, .. } => {
                    let title = match input_type {
                        DateInputType::Scheduled => "Select SCHEDULED Date & Time",
                        DateInputType::Deadline => "Select DEADLINE Date & Time",
                    };

                    // Render calendar and time
                    let mut calendar_lines = App::render_calendar(*viewing_month, *selected_date);
                    calendar_lines.push(Line::from(""));

                    // Show time input
                    let time_display = if *editing_time {
                        format!("Time: [{:02}:{:02}] (editing)", hour, minute)
                    } else {
                        format!("Time: {:02}:{:02}", hour, minute)
                    };
                    calendar_lines.push(Line::from(time_display));

                    let calendar_widget = Paragraph::new(calendar_lines)
                        .block(Block::default().borders(Borders::ALL).title(title))
                        .style(Style::default());
                    f.render_widget(calendar_widget, chunks[0]);

                    let status = if *editing_time {
                        Paragraph::new("↑/↓: Hours | ←/→: Minutes | Tab: Switch to Date | Enter: Confirm | Esc: Cancel")
                    } else {
                        Paragraph::new("Arrows: Navigate Date | </> or Page Up/Down: Change Month | Tab: Switch to Time | Enter: Confirm | Esc: Cancel")
                    };
                    let status = status.block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Gray));
                    f.render_widget(status, chunks[1]);
                }
                Mode::Editor { textarea, .. } => {
                    // TODO editor (editable)
                    f.render_widget(textarea, chunks[0]);

                    let status = Paragraph::new("Esc or Ctrl+S: Save & Exit | Normal editing keys work")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Yellow));
                    f.render_widget(status, chunks[1]);
                }
                Mode::TagManagement { tag_input, .. } => {
                    // Tag management
                    f.render_widget(tag_input, chunks[0]);

                    let status = Paragraph::new("Enter: Save tags (space-separated) | Esc: Cancel | Type tags like: work urgent home")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Cyan));
                    f.render_widget(status, chunks[1]);
                }
                Mode::QuickCapture { title_input } => {
                    // Quick capture
                    f.render_widget(title_input, chunks[0]);

                    let status = Paragraph::new("Enter: Create TODO scheduled for today | Esc: Cancel | Type the task title")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Green));
                    f.render_widget(status, chunks[1]);
                }
            }
        })?;

        if let Event::Key(key) = event::read()? {
            match &mut app.mode {
                Mode::Browser { todos, selected, filter } => {
                    let filtered_len = App::filter_todos(todos, filter).len();
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Tab => {
                            app.toggle_view_filter();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if *selected < filtered_len.saturating_sub(1) {
                                *selected += 1;
                            }
                        }
                        KeyCode::Enter => {
                            app.open_todo()?;
                        }
                        KeyCode::Char('t') => {
                            app.toggle_todo_state_from_browser()?;
                        }
                        KeyCode::Char('s') => {
                            app.enter_date_input_from_browser(DateInputType::Scheduled)?;
                        }
                        KeyCode::Char('d') => {
                            app.enter_date_input_from_browser(DateInputType::Deadline)?;
                        }
                        KeyCode::Char('e') => {
                            app.enter_edit_mode_from_browser()?;
                        }
                        KeyCode::Char('g') => {
                            app.enter_tag_management()?;
                        }
                        KeyCode::Char('c') => {
                            app.enter_quick_capture()?;
                        }
                        KeyCode::Char('n') => {
                            app.create_new_note()?;
                        }
                        KeyCode::Char('x') | KeyCode::Delete => {
                            app.delete_todo_from_browser()?;
                        }
                        _ => {}
                    }
                }
                Mode::Viewer { scroll, .. } => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('t') => {
                        app.toggle_todo_state()?;
                    }
                    KeyCode::Char('s') => {
                        app.enter_date_input(DateInputType::Scheduled)?;
                    }
                    KeyCode::Char('d') => {
                        app.enter_date_input(DateInputType::Deadline)?;
                    }
                    KeyCode::Char('e') => {
                        app.enter_edit_mode()?;
                    }
                    KeyCode::Esc => {
                        app.back_to_browser()?;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *scroll = scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *scroll = scroll.saturating_add(1);
                    }
                    _ => {}
                },
                Mode::DateInput { editing_time, .. } => {
                    match key.code {
                        KeyCode::Enter => {
                            app.submit_date_input()?;
                        }
                        KeyCode::Esc => {
                            app.cancel_date_input()?;
                        }
                        KeyCode::Tab => {
                            app.toggle_time_edit();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if *editing_time {
                                app.adjust_time(1, 0);
                            } else {
                                app.calendar_move_day(-7);
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if *editing_time {
                                app.adjust_time(-1, 0);
                            } else {
                                app.calendar_move_day(7);
                            }
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            if *editing_time {
                                app.adjust_time(0, -1);
                            } else {
                                app.calendar_move_day(-1);
                            }
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            if *editing_time {
                                app.adjust_time(0, 1);
                            } else {
                                app.calendar_move_day(1);
                            }
                        }
                        KeyCode::Char('<') | KeyCode::PageUp => {
                            if !*editing_time {
                                app.calendar_change_month(-1);
                            }
                        }
                        KeyCode::Char('>') | KeyCode::PageDown => {
                            if !*editing_time {
                                app.calendar_change_month(1);
                            }
                        }
                        _ => {}
                    }
                }
                Mode::Editor { textarea, .. } => {
                    match key.code {
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.exit_edit_mode_with_save()?;
                        }
                        KeyCode::Esc => {
                            app.exit_edit_mode_with_save()?;
                        }
                        _ => {
                            // Pass all other keys to the textarea
                            textarea.input(key);
                        }
                    }
                }
                Mode::TagManagement { tag_input, .. } => {
                    match key.code {
                        KeyCode::Enter => {
                            app.save_tags()?;
                        }
                        KeyCode::Esc => {
                            app.back_to_browser()?;
                        }
                        _ => {
                            // Pass all other keys to the tag input
                            tag_input.input(key);
                        }
                    }
                }
                Mode::QuickCapture { title_input } => {
                    match key.code {
                        KeyCode::Enter => {
                            app.save_quick_capture()?;
                        }
                        KeyCode::Esc => {
                            app.back_to_browser()?;
                        }
                        _ => {
                            // Pass all other keys to the title input
                            title_input.input(key);
                        }
                    }
                }
            }
        }
    }
}
