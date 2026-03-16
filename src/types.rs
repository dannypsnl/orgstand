use chrono::NaiveDate;
use std::path::PathBuf;
use tui_textarea::TextArea;

#[derive(Clone)]
pub struct TodoEntry {
    pub keyword: String,
    pub title: String,
    pub tags: Vec<String>,
    pub file_path: PathBuf,
    pub content: String,
    pub level: usize,
}

#[derive(Clone, PartialEq)]
pub enum ViewFilter {
    All,
    Today,
}

#[derive(Clone)]
pub enum DateInputType {
    Scheduled,
    Deadline,
    Plain,
}

pub enum Mode {
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
        viewing_month: NaiveDate,
        hour: u32,
        minute: u32,
        editing_time: bool,
    },
    TagManagement {
        todo: TodoEntry,
        tags: Vec<String>,
        selected: usize,
        editing: Option<TextArea<'static>>,
    },
    QuickCapture {
        title_input: TextArea<'static>,
    },
    Help {
        scroll: u16,
    },
}
