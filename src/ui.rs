use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::dates::get_display_date;
use crate::types::{DateInputType, Mode, ViewFilter};

pub fn render_calendar(viewing_month: NaiveDate, selected_date: NaiveDate) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let header = format!("{}", viewing_month.format("%B %Y"));
    lines.push(Line::from(vec![Span::styled(
        header,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    lines.push(Line::from("Su  Mo  Tu  We  Th  Fr  Sa"));
    lines.push(Line::from("───────────────────────────"));

    let first_day =
        NaiveDate::from_ymd_opt(viewing_month.year(), viewing_month.month(), 1).unwrap();
    let first_weekday = first_day.weekday().num_days_from_sunday();

    let next_month = if viewing_month.month() == 12 {
        NaiveDate::from_ymd_opt(viewing_month.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(viewing_month.year(), viewing_month.month() + 1, 1).unwrap()
    };
    let last_day = next_month.pred_opt().unwrap().day();

    let mut week_line = String::new();
    for _ in 0..first_weekday {
        week_line.push_str("    ");
    }

    for day in 1..=last_day {
        let current_date =
            NaiveDate::from_ymd_opt(viewing_month.year(), viewing_month.month(), day).unwrap();
        let day_str = format!("{:2}", day);

        if current_date == selected_date {
            week_line.push_str(&format!("[{}]", day_str));
        } else if current_date == Local::now().date_naive() {
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
    lines.push(Line::from(format!(
        "Selected: {}",
        selected_date.format("%Y-%m-%d %a")
    )));

    lines
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    match &app.mode {
        Mode::Browser {
            todos,
            selected,
            filter,
        } => {
            let filtered_todos = App::filter_todos(todos, filter);

            let items: Vec<ListItem> = if matches!(filter, ViewFilter::Today) {
                let mut result = Vec::new();

                let today = Local::now().date_naive();
                let weekday = today.weekday().num_days_from_monday();
                let week_start = today - chrono::Duration::days(weekday as i64);

                for day_offset in 0..7 {
                    let current_day = week_start + chrono::Duration::days(day_offset);

                    let header = current_day.format("%A, %d %b, %Y").to_string();
                    let header_style = if current_day == today {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    };
                    result.push(
                        ListItem::new(format!("── {} ──", header)).style(header_style),
                    );

                    let day_todos: Vec<_> = filtered_todos
                        .iter()
                        .filter(|t| get_display_date(t) == Some(current_day))
                        .collect();

                    if day_todos.is_empty() {
                        result.push(
                            ListItem::new("  (no entries)")
                                .style(Style::default().fg(Color::DarkGray)),
                        );
                    } else {
                        for todo in day_todos {
                            let tags_str = if !todo.tags.is_empty() {
                                format!(" :{}: ", todo.tags.join(":"))
                            } else {
                                String::new()
                            };

                            let actual_index = filtered_todos
                                .iter()
                                .position(|t| {
                                    t.title == todo.title && t.file_path == todo.file_path
                                })
                                .unwrap_or(0);

                            let is_selected = actual_index == *selected;

                            let mut spans = vec![Span::raw("  ")];

                            if !todo.keyword.is_empty() {
                                let keyword_style = keyword_style(is_selected, &todo.keyword);
                                spans.push(Span::styled(
                                    format!("[{}] ", todo.keyword),
                                    keyword_style,
                                ));
                            }

                            let rest_style = if is_selected {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::White)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            };

                            spans.push(Span::styled(
                                format!(
                                    "{}{}  - {}",
                                    todo.title,
                                    tags_str,
                                    todo.file_path
                                        .file_name()
                                        .unwrap()
                                        .to_string_lossy()
                                ),
                                rest_style,
                            ));

                            result.push(ListItem::new(Line::from(spans)));
                        }
                    }
                }
                result
            } else {
                filtered_todos
                    .iter()
                    .enumerate()
                    .map(|(i, todo)| {
                        let tags_str = if !todo.tags.is_empty() {
                            format!(" :{}: ", todo.tags.join(":"))
                        } else {
                            String::new()
                        };

                        let is_selected = i == *selected;

                        let mut spans = Vec::new();

                        if !todo.keyword.is_empty() {
                            let kstyle = keyword_style(is_selected, &todo.keyword);
                            spans.push(Span::styled(format!("[{}] ", todo.keyword), kstyle));
                        }

                        let rest_style = if is_selected {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };

                        spans.push(Span::styled(
                            format!(
                                "{}{}  - {}",
                                todo.title,
                                tags_str,
                                todo.file_path.file_name().unwrap().to_string_lossy()
                            ),
                            rest_style,
                        ));

                        ListItem::new(Line::from(spans))
                    })
                    .collect()
            };

            let view_mode = match filter {
                ViewFilter::All => "All TODOs",
                ViewFilter::Today => "Week Agenda",
            };

            let list = List::new(items).block(
                Block::default().borders(Borders::ALL).title(format!(
                    "{} ({}/{}) - {}",
                    view_mode,
                    filtered_todos.len(),
                    todos.len(),
                    app.directory.display()
                )),
            );
            f.render_widget(list, chunks[0]);

            let status = Paragraph::new("↑/↓: Navigate | Enter: View | t: Toggle | s: Schedule | d: Deadline | e: Edit | g: Tags | c: Capture | n: Note | x: Delete | Tab: View | q: Quit")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Gray));
            f.render_widget(status, chunks[1]);
        }

        Mode::Viewer { todo, scroll } => {
            let title_style = if todo.keyword == "TODO" {
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD)
            } else if todo.keyword == "DONE" {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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

            let status = Paragraph::new(
                "t: Toggle TODO/DONE | s: Schedule | d: Deadline | e: Edit | Esc: Back | q: Quit",
            )
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));
            f.render_widget(status, chunks[1]);
        }

        Mode::DateInput {
            input_type,
            selected_date,
            viewing_month,
            hour,
            minute,
            editing_time,
            ..
        } => {
            let title = match input_type {
                DateInputType::Scheduled => "Select SCHEDULED Date & Time",
                DateInputType::Deadline => "Select DEADLINE Date & Time",
                DateInputType::Plain => "Select Date & Time (Plain)",
            };

            let mut calendar_lines = render_calendar(*viewing_month, *selected_date);
            calendar_lines.push(Line::from(""));

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
            let status = status
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Gray));
            f.render_widget(status, chunks[1]);
        }

        Mode::Editor { textarea, .. } => {
            f.render_widget(textarea, chunks[0]);

            let status = Paragraph::new("Esc or Ctrl+S: Save & Exit | Normal editing keys work")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(status, chunks[1]);
        }

        Mode::TagManagement {
            todo,
            tags,
            selected,
            editing,
        } => {
            if let Some(textarea) = editing {
                f.render_widget(textarea, chunks[0]);
                let status = Paragraph::new("Enter: Save tag | Esc: Cancel")
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::Cyan));
                f.render_widget(status, chunks[1]);
            } else {
                if tags.is_empty() {
                    let help_text = vec![
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "  No tags yet.",
                            Style::default().fg(Color::Yellow),
                        )]),
                        Line::from(""),
                        Line::from(vec![Span::styled(
                            "  Press 'a' to add a new tag!",
                            Style::default().fg(Color::Cyan),
                        )]),
                    ];
                    let help = Paragraph::new(help_text).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!("Tags for: {}", todo.title)),
                    );
                    f.render_widget(help, chunks[0]);
                } else {
                    let items: Vec<ListItem> = tags
                        .iter()
                        .enumerate()
                        .map(|(i, tag)| {
                            let display = format!("  {}", tag);
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
                            .title(format!("Tags for: {}", todo.title)),
                    );
                    f.render_widget(list, chunks[0]);
                }

                let status = Paragraph::new(
                    "↑/↓: Navigate | Enter: Edit tag | a: Add tag | x/Delete: Remove tag | Esc: Save & Exit",
                )
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan));
                f.render_widget(status, chunks[1]);
            }
        }

        Mode::QuickCapture { title_input } => {
            f.render_widget(title_input, chunks[0]);

            let status = Paragraph::new(
                "Enter: Create TODO scheduled for today | Esc: Cancel | Type the task title",
            )
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Green));
            f.render_widget(status, chunks[1]);
        }

        Mode::Help { scroll } => {
            let help_text = vec![
                Line::from(vec![Span::styled(
                    "OrgStand - Keybindings Help",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Browser Mode (All TODOs / Week Agenda) ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  q             Quit the application"),
                Line::from("  ? or h        Show this help screen"),
                Line::from("  Tab           Switch between All TODOs and Week Agenda"),
                Line::from("  ↑/k or ↓/j    Navigate up/down in the list"),
                Line::from("  Enter         Open selected TODO in viewer"),
                Line::from("  t             Toggle TODO state (TODO ↔ DONE)"),
                Line::from("  s             Set/edit SCHEDULED date"),
                Line::from("  d             Set/edit DEADLINE date"),
                Line::from("  p             Set/edit plain date"),
                Line::from("  e             Edit TODO content in editor"),
                Line::from("  g             Manage tags"),
                Line::from("  c             Quick capture (create TODO for today)"),
                Line::from("  n             Create new note"),
                Line::from("  x or Delete   Delete TODO"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Viewer Mode ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  q             Return to browser"),
                Line::from("  Esc           Return to browser"),
                Line::from("  ↑/k or ↓/j    Scroll up/down"),
                Line::from("  t             Toggle TODO state"),
                Line::from("  s/d/p         Set dates (same as browser)"),
                Line::from("  e             Edit content"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Date Input Mode ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  Arrows        Navigate calendar (when editing date)"),
                Line::from("  </> PageUp/Dn Change month"),
                Line::from("  Tab           Switch between date and time editing"),
                Line::from("  ↑/↓           Adjust hours (when editing time)"),
                Line::from("  ←/→           Adjust minutes (when editing time)"),
                Line::from("  Enter         Confirm and save"),
                Line::from("  Esc           Cancel"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Editor Mode ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  Esc or Ctrl+S Save and exit"),
                Line::from("  Normal keys   Edit text"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Tag Management ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  Enter         Save tags"),
                Line::from("  Esc           Cancel"),
                Line::from("  Type          Edit tags (format: :tag1:tag2:)"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Quick Capture ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  Enter         Create TODO"),
                Line::from("  Esc           Cancel"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "== Week Agenda View ==",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from("  Shows all days from Monday to Sunday of the current week"),
                Line::from("  Each day has a header (e.g., Tuesday, 21 Jan, 2026)"),
                Line::from("  Today's header is highlighted in yellow"),
                Line::from("  Days without entries show \"(no entries)\""),
                Line::from(""),
                Line::from("Press q, Esc, or ? to close this help screen"),
            ];

            let help_widget = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Help"))
                .scroll((*scroll, 0))
                .style(Style::default());
            f.render_widget(help_widget, chunks[0]);

            let status = Paragraph::new("↑/k or ↓/j: Scroll | q/Esc/?: Close")
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Gray));
            f.render_widget(status, chunks[1]);
        }
    }
}

fn keyword_style(is_selected: bool, keyword: &str) -> Style {
    if is_selected {
        if keyword == "TODO" {
            Style::default()
                .fg(Color::Red)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if keyword == "DONE" {
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        }
    } else if keyword == "TODO" {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if keyword == "DONE" {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    }
}
