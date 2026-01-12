use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
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
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("org") {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
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
                        "File: {} | ↑/↓: Scroll | e: Edit | Esc: Back | q: Quit",
                        todo.file_path.display()
                    );
                    let status = Paragraph::new(status_text)
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
