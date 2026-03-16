mod app;
mod dates;
mod parser;
mod scanner;
mod types;
mod ui;
mod writer;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::App;
use types::{DateInputType, Mode};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path_arg = args.get(1).cloned();

    let mut app = App::new(path_arg)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app);

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
        terminal.draw(|f| ui::draw(f, app))?;

        let event = event::read()?;

        // Handle mouse events for TagManagement
        if let Event::Mouse(mouse_event) = event {
            if let Mode::TagManagement {
                tags,
                selected,
                editing,
                ..
            } = &mut app.mode
            {
                if editing.is_none() {
                    use crossterm::event::MouseEventKind;
                    if matches!(mouse_event.kind, MouseEventKind::Down(_)) {
                        let click_y = mouse_event.row as usize;
                        if click_y >= 2 && click_y < 2 + tags.len() {
                            let clicked_index = click_y - 2;
                            *selected = clicked_index;

                            let current_tag = tags[clicked_index].clone();
                            let mut textarea =
                                tui_textarea::TextArea::new(vec![current_tag]);
                            textarea.set_block(
                                ratatui::widgets::Block::default()
                                    .borders(ratatui::widgets::Borders::ALL)
                                    .title("Edit Tag"),
                            );
                            *editing = Some(textarea);
                        }
                    }
                }
            }
        }

        if let Event::Key(key) = event {
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
                        KeyCode::Char('p') => {
                            app.enter_date_input_from_browser(DateInputType::Plain)?;
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
                        KeyCode::Char('?') | KeyCode::Char('h') => {
                            app.enter_help();
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
                    KeyCode::Char('p') => {
                        app.enter_date_input(DateInputType::Plain)?;
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
                        KeyCode::Char('s')
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            app.exit_edit_mode_with_save()?;
                        }
                        KeyCode::Esc => {
                            app.exit_edit_mode_with_save()?;
                        }
                        _ => {
                            textarea.input(key);
                        }
                    }
                }
                Mode::TagManagement {
                    tags,
                    selected,
                    editing,
                    ..
                } => {
                    if let Some(textarea) = editing {
                        match key.code {
                            KeyCode::Enter => {
                                let new_tag = textarea.lines().join("");
                                if !new_tag.is_empty() {
                                    tags[*selected] = new_tag;
                                }
                                *editing = None;
                            }
                            KeyCode::Esc => {
                                *editing = None;
                            }
                            _ => {
                                textarea.input(key);
                            }
                        }
                    } else {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                *selected = selected.saturating_sub(1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if *selected < tags.len().saturating_sub(1) {
                                    *selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if *selected < tags.len() {
                                    let current_tag = tags[*selected].clone();
                                    let mut textarea =
                                        tui_textarea::TextArea::new(vec![current_tag]);
                                    textarea.set_block(
                                        ratatui::widgets::Block::default()
                                            .borders(ratatui::widgets::Borders::ALL)
                                            .title("Edit Tag"),
                                    );
                                    *editing = Some(textarea);
                                }
                            }
                            KeyCode::Char('a') | KeyCode::Char('n') => {
                                tags.push(String::new());
                                *selected = tags.len().saturating_sub(1);
                                let mut textarea =
                                    tui_textarea::TextArea::new(vec![String::new()]);
                                textarea.set_block(
                                    ratatui::widgets::Block::default()
                                        .borders(ratatui::widgets::Borders::ALL)
                                        .title("New Tag"),
                                );
                                *editing = Some(textarea);
                            }
                            KeyCode::Char('x') | KeyCode::Delete => {
                                if *selected < tags.len() && !tags.is_empty() {
                                    tags.remove(*selected);
                                    if *selected >= tags.len() && !tags.is_empty() {
                                        *selected = tags.len() - 1;
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                app.save_tags()?;
                            }
                            _ => {}
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
                            title_input.input(key);
                        }
                    }
                }
                Mode::Help { scroll } => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
                            app.back_to_browser()?;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            *scroll = scroll.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *scroll = scroll.saturating_add(1);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
