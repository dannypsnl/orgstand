use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use ratatui::widgets::{Block, Borders};
use std::path::{Path, PathBuf};
use tui_textarea::TextArea;

use crate::dates::{parse_existing_date, parse_existing_time};
use crate::parser::extract_all_todos;
use crate::types::{DateInputType, Mode, TodoEntry, ViewFilter};
use crate::writer::{
    add_date_to_file, append_new_note, append_quick_capture, delete_entry_from_file,
    toggle_keyword_in_file, update_tags_in_file, update_todo_in_file,
};

pub struct App {
    pub mode: Mode,
    pub directory: PathBuf,
    pub last_filter: ViewFilter,
}

impl App {
    pub fn new(path_arg: Option<String>) -> Result<Self> {
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

        let todos = extract_all_todos(&directory)?;

        Ok(App {
            mode: Mode::Browser {
                todos,
                selected: 0,
                filter: ViewFilter::Today,
            },
            directory,
            last_filter: ViewFilter::Today,
        })
    }

    pub fn filter_todos(todos: &[TodoEntry], filter: &ViewFilter) -> Vec<TodoEntry> {
        let mut filtered: Vec<TodoEntry> = match filter {
            ViewFilter::All => todos
                .iter()
                .filter(|todo| !todo.keyword.is_empty() && todo.keyword != "DONE")
                .cloned()
                .collect(),
            ViewFilter::Today => {
                let today = Local::now().date_naive();
                let weekday = today.weekday().num_days_from_monday();
                let week_start = today - chrono::Duration::days(weekday as i64);
                let week_end = week_start + chrono::Duration::days(6);

                todos
                    .iter()
                    .filter(|todo| {
                        let date_to_check =
                            parse_existing_date(&todo.content, &DateInputType::Scheduled)
                                .or_else(|| {
                                    parse_existing_date(&todo.content, &DateInputType::Deadline)
                                })
                                .or_else(|| crate::dates::parse_any_date(&todo.content));

                        if let Some(date) = date_to_check {
                            return date >= week_start && date <= week_end;
                        }
                        false
                    })
                    .cloned()
                    .collect()
            }
        };

        filtered.sort_by_key(|todo| {
            parse_existing_date(&todo.content, &DateInputType::Scheduled)
                .or_else(|| crate::dates::parse_any_date(&todo.content))
        });

        filtered
    }

    pub fn toggle_view_filter(&mut self) {
        if let Mode::Browser { todos, filter, .. } = &self.mode {
            let new_filter = match filter {
                ViewFilter::All => ViewFilter::Today,
                ViewFilter::Today => ViewFilter::All,
            };
            self.last_filter = new_filter.clone();
            self.mode = Mode::Browser {
                todos: todos.clone(),
                selected: 0,
                filter: new_filter,
            };
        }
    }

    pub fn back_to_browser(&mut self) -> Result<()> {
        let todos = extract_all_todos(&self.directory)?;
        self.mode = Mode::Browser {
            todos,
            selected: 0,
            filter: self.last_filter.clone(),
        };
        Ok(())
    }

    pub fn enter_help(&mut self) {
        self.mode = Mode::Help { scroll: 0 };
    }

    pub fn open_todo(&mut self) -> Result<()> {
        if let Mode::Browser {
            todos,
            selected,
            filter,
        } = &self.mode
        {
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

    pub fn enter_edit_mode(&mut self) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let mut textarea =
                TextArea::new(todo.content.lines().map(|s| s.to_string()).collect());
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

    pub fn exit_edit_mode_with_save(&mut self) -> Result<()> {
        let (todo_clone, new_content) = if let Mode::Editor { todo, textarea } = &self.mode {
            let content = textarea.lines().join("\n");
            (todo.clone(), content)
        } else {
            return Ok(());
        };

        update_todo_in_file(&todo_clone.file_path, &todo_clone, &new_content)?;
        self.back_to_browser()?;
        Ok(())
    }

    pub fn toggle_todo_state(&mut self) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let new_keyword = if todo.keyword == "TODO" { "DONE" } else { "TODO" };
            toggle_keyword_in_file(todo, new_keyword)?;

            let todos = extract_all_todos(&self.directory)?;
            if let Some(updated) = todos
                .iter()
                .find(|t| t.title == todo.title && t.file_path == todo.file_path)
            {
                self.mode = Mode::Viewer {
                    todo: updated.clone(),
                    scroll: 0,
                };
            }
        }
        Ok(())
    }

    pub fn enter_date_input(&mut self, input_type: DateInputType) -> Result<()> {
        if let Mode::Viewer { todo, .. } = &self.mode {
            let today = Local::now().date_naive();
            let initial_date =
                parse_existing_date(&todo.content, &input_type).unwrap_or(today);
            let (hour, minute) = parse_existing_time(&todo.content, &input_type);

            self.mode = Mode::DateInput {
                todo: todo.clone(),
                input_type,
                selected_date: initial_date,
                viewing_month: NaiveDate::from_ymd_opt(
                    initial_date.year(),
                    initial_date.month(),
                    1,
                )
                .unwrap(),
                hour,
                minute,
                editing_time: false,
            };
        }
        Ok(())
    }

    pub fn cancel_date_input(&mut self) -> Result<()> {
        if let Mode::DateInput { todo, .. } = &self.mode {
            self.mode = Mode::Viewer {
                todo: todo.clone(),
                scroll: 0,
            };
        }
        Ok(())
    }

    pub fn submit_date_input(&mut self) -> Result<()> {
        let (todo_clone, input_type_clone, selected_date, hour, minute) =
            if let Mode::DateInput {
                todo,
                input_type,
                selected_date,
                hour,
                minute,
                ..
            } = &self.mode
            {
                (todo.clone(), input_type.clone(), *selected_date, *hour, *minute)
            } else {
                return Ok(());
            };

        let weekday = selected_date.format("%a");
        let date_str = format!(
            "{} {} {:02}:{:02}",
            selected_date.format("%Y-%m-%d"),
            weekday,
            hour,
            minute
        );

        self.mode = Mode::Viewer {
            todo: todo_clone.clone(),
            scroll: 0,
        };

        add_date_to_file(&todo_clone, &date_str, &input_type_clone)?;
        self.back_to_browser()?;
        Ok(())
    }

    pub fn calendar_move_day(&mut self, days: i64) {
        if let Mode::DateInput {
            selected_date,
            viewing_month,
            editing_time,
            ..
        } = &mut self.mode
        {
            if !*editing_time {
                if let Some(new_date) =
                    selected_date.checked_add_signed(chrono::Duration::days(days))
                {
                    *selected_date = new_date;

                    if selected_date.year() != viewing_month.year()
                        || selected_date.month() != viewing_month.month()
                    {
                        *viewing_month = NaiveDate::from_ymd_opt(
                            selected_date.year(),
                            selected_date.month(),
                            1,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }

    pub fn adjust_time(&mut self, hours_delta: i32, minutes_delta: i32) {
        if let Mode::DateInput {
            hour,
            minute,
            editing_time,
            ..
        } = &mut self.mode
        {
            if *editing_time {
                *hour = ((*hour as i32 + hours_delta).rem_euclid(24)) as u32;
                *minute = ((*minute as i32 + minutes_delta).rem_euclid(60)) as u32;
            }
        }
    }

    pub fn toggle_time_edit(&mut self) {
        if let Mode::DateInput { editing_time, .. } = &mut self.mode {
            *editing_time = !*editing_time;
        }
    }

    pub fn calendar_change_month(&mut self, months: i32) {
        if let Mode::DateInput {
            selected_date,
            viewing_month,
            ..
        } = &mut self.mode
        {
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

                if selected_date.year() != final_year || selected_date.month() != final_month {
                    *selected_date = new_viewing;
                }
            }
        }
    }

    fn get_selected_todo_from_browser(&self) -> Option<TodoEntry> {
        if let Mode::Browser {
            todos,
            selected,
            filter,
        } = &self.mode
        {
            let filtered_todos = Self::filter_todos(todos, filter);
            filtered_todos.get(*selected).cloned()
        } else {
            None
        }
    }

    pub fn toggle_todo_state_from_browser(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let new_keyword = if todo.keyword == "TODO" { "DONE" } else { "TODO" };
            toggle_keyword_in_file(&todo, new_keyword)?;
            self.back_to_browser()?;
        }
        Ok(())
    }

    pub fn enter_date_input_from_browser(&mut self, input_type: DateInputType) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let today = Local::now().date_naive();
            let initial_date =
                parse_existing_date(&todo.content, &input_type).unwrap_or(today);
            let (hour, minute) = parse_existing_time(&todo.content, &input_type);

            self.mode = Mode::DateInput {
                todo,
                input_type,
                selected_date: initial_date,
                viewing_month: NaiveDate::from_ymd_opt(
                    initial_date.year(),
                    initial_date.month(),
                    1,
                )
                .unwrap(),
                hour,
                minute,
                editing_time: false,
            };
        }
        Ok(())
    }

    pub fn enter_edit_mode_from_browser(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let mut textarea =
                TextArea::new(todo.content.lines().map(|s| s.to_string()).collect());
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Editing: [{}] {}", todo.keyword, todo.title)),
            );
            self.mode = Mode::Editor { todo, textarea };
        }
        Ok(())
    }

    pub fn delete_todo_from_browser(&mut self) -> Result<()> {
        let todo = match self.get_selected_todo_from_browser() {
            Some(t) => t,
            None => return Ok(()),
        };

        delete_entry_from_file(&todo)?;
        self.back_to_browser()
    }

    pub fn create_new_note(&mut self) -> Result<()> {
        append_new_note(&self.directory)?;
        self.back_to_browser()?;
        Ok(())
    }

    pub fn enter_quick_capture(&mut self) -> Result<()> {
        let mut title_input = TextArea::new(vec![String::new()]);
        title_input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Quick Capture - Enter title (will be scheduled for today)"),
        );
        self.mode = Mode::QuickCapture { title_input };
        Ok(())
    }

    pub fn save_quick_capture(&mut self) -> Result<()> {
        let title = if let Mode::QuickCapture { title_input } = &self.mode {
            title_input.lines().join(" ").trim().to_string()
        } else {
            return Ok(());
        };

        if title.is_empty() {
            self.back_to_browser()?;
            return Ok(());
        }

        append_quick_capture(&self.directory, &title)?;
        self.back_to_browser()?;
        Ok(())
    }

    pub fn enter_tag_management(&mut self) -> Result<()> {
        if let Some(todo) = self.get_selected_todo_from_browser() {
            let tags = todo.tags.clone();
            self.mode = Mode::TagManagement {
                todo,
                tags,
                selected: 0,
                editing: None,
            };
        }
        Ok(())
    }

    pub fn save_tags(&mut self) -> Result<()> {
        let (todo_clone, new_tags) = if let Mode::TagManagement { todo, tags, .. } = &self.mode {
            (todo.clone(), tags.clone())
        } else {
            return Ok(());
        };

        update_tags_in_file(&todo_clone, &new_tags)?;
        self.back_to_browser()?;
        Ok(())
    }
}
