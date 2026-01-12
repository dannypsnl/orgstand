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
    title: String,        // The title text
    file_path: PathBuf,   // Which file it's in
    content: String,      // Full content of this entry (including children)
    level: usize,
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
    },
}

struct App {
    mode: Mode,
    directory: PathBuf,
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
            mode: Mode::Browser { todos, selected: 0 },
            directory,
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
        let mut todos = Vec::new();

        // Split by lines and find TODO entries
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Check if this line is a heading with TODO keyword
            if line.starts_with('*') {
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() >= 3 {
                    let stars = parts[0];
                    let potential_keyword = parts[1];

                    // Check if it's a TODO keyword
                    if matches!(
                        potential_keyword,
                        "TODO" | "DONE" | "NEXT" | "WAITING" | "CANCELLED" | "CANCELED"
                    ) {
                        let level = stars.chars().count();
                        let keyword = potential_keyword.to_string();
                        let title = parts[2].to_string();

                        // Extract content until next heading of same or higher level
                        let mut content = String::new();
                        content.push_str(line);
                        content.push('\n');

                        let mut j = i + 1;
                        while j < lines.len() {
                            let next_line = lines[j];
                            if next_line.starts_with('*') {
                                let next_level = next_line.chars().take_while(|c| *c == '*').count();
                                if next_level <= level {
                                    break; // Found next section at same or higher level
                                }
                            }
                            content.push_str(next_line);
                            content.push('\n');
                            j += 1;
                        }

                        todos.push(TodoEntry {
                            keyword,
                            title,
                            file_path: file_path.to_path_buf(),
                            content,
                            level,
                        });

                        i = j; // Skip to next section
                        continue;
                    }
                }
            }
            i += 1;
        }

        todos
    }

    fn open_todo(&mut self) -> Result<()> {
        if let Mode::Browser { todos, selected } = &self.mode {
            if let Some(todo) = todos.get(*selected) {
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
            self.mode = Mode::Browser { todos, selected: 0 };
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
        self.mode = Mode::Browser { todos, selected: 0 };
        Ok(())
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

            // Try to parse existing date from todo content
            let initial_date = Self::parse_existing_date(&todo.content, &input_type).unwrap_or(today);

            self.mode = Mode::DateInput {
                todo: todo.clone(),
                input_type,
                selected_date: initial_date,
                viewing_month: NaiveDate::from_ymd_opt(initial_date.year(), initial_date.month(), 1).unwrap(),
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

    fn submit_date_input(&mut self) -> Result<()> {
        // Clone data first to avoid borrow conflicts
        let (todo_clone, input_type_clone, selected_date) = if let Mode::DateInput { todo, input_type, selected_date, .. } = &self.mode {
            (todo.clone(), input_type.clone(), *selected_date)
        } else {
            return Ok(());
        };

        // Format date as org-mode date string with day of week
        let weekday = selected_date.format("%a");
        let date_str = format!("{} {}", selected_date.format("%Y-%m-%d"), weekday);

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
        if let Mode::DateInput { selected_date, viewing_month, .. } = &mut self.mode {
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

            // Reload
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

            // Reload
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

        // Read back and find updated todo
        let updated_content = std::fs::read_to_string(&todo_clone.file_path)?;
        let todos = Self::extract_todos_from_content(&updated_content, &todo_clone.file_path);

        // Try to find the updated version, otherwise use the new content directly
        let updated_todo = todos
            .iter()
            .find(|t| t.title == todo_clone.title && t.keyword == todo_clone.keyword)
            .cloned()
            .unwrap_or_else(|| TodoEntry {
                keyword: todo_clone.keyword.clone(),
                title: todo_clone.title.clone(),
                file_path: todo_clone.file_path.clone(),
                content: new_content,
                level: todo_clone.level,
            });

        // Now we can safely update mode
        self.mode = Mode::Viewer {
            todo: updated_todo,
            scroll: 0,
        };

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
                Mode::Browser { todos, selected } => {
                    // TODO entries browser
                    let items: Vec<ListItem> = todos
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

                            let display = format!(
                                "[{}] {} - {}",
                                todo.keyword,
                                todo.title,
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

                    let list = List::new(items).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!("TODO Entries ({}) - {}", todos.len(), app.directory.display())),
                    );
                    f.render_widget(list, chunks[0]);

                    let status = Paragraph::new("↑/↓: Navigate | Enter: Open | q: Quit")
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
                        "t: Toggle TODO/DONE | s: Schedule | D: Deadline | e: Edit | Esc: Back | q: Quit"
                    );
                    let status = Paragraph::new(status_text)
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Gray));
                    f.render_widget(status, chunks[1]);
                }
                Mode::DateInput { input_type, selected_date, viewing_month, .. } => {
                    let title = match input_type {
                        DateInputType::Scheduled => "Select SCHEDULED Date",
                        DateInputType::Deadline => "Select DEADLINE Date",
                    };

                    // Render calendar
                    let calendar_lines = App::render_calendar(*viewing_month, *selected_date);
                    let calendar_widget = Paragraph::new(calendar_lines)
                        .block(Block::default().borders(Borders::ALL).title(title))
                        .style(Style::default());
                    f.render_widget(calendar_widget, chunks[0]);

                    let status = Paragraph::new("Arrows: Navigate | </> or Page Up/Down: Change Month | Enter: Confirm | Esc: Cancel")
                        .block(Block::default().borders(Borders::ALL))
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
            }
        })?;

        if let Event::Key(key) = event::read()? {
            match &mut app.mode {
                Mode::Browser { todos, selected } => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if *selected < todos.len().saturating_sub(1) {
                            *selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        app.open_todo()?;
                    }
                    _ => {}
                },
                Mode::Viewer { scroll, .. } => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('t') => {
                        app.toggle_todo_state()?;
                    }
                    KeyCode::Char('s') => {
                        app.enter_date_input(DateInputType::Scheduled)?;
                    }
                    KeyCode::Char('D') => {
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
                Mode::DateInput { .. } => match key.code {
                    KeyCode::Enter => {
                        app.submit_date_input()?;
                    }
                    KeyCode::Esc => {
                        app.cancel_date_input()?;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.calendar_move_day(-7);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.calendar_move_day(7);
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        app.calendar_move_day(-1);
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        app.calendar_move_day(1);
                    }
                    KeyCode::Char('<') | KeyCode::PageUp => {
                        app.calendar_change_month(-1);
                    }
                    KeyCode::Char('>') | KeyCode::PageDown => {
                        app.calendar_change_month(1);
                    }
                    _ => {}
                },
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
            }
        }
    }
}
