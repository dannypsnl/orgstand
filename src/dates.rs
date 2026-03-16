use chrono::{Local, NaiveDate};

use crate::types::{DateInputType, TodoEntry};

pub fn parse_org_timestamp(timestamp: &str) -> Option<(NaiveDate, Option<(u32, u32)>)> {
    let parts: Vec<&str> = timestamp.split_whitespace().collect();
    let date = parts
        .first()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())?;

    let time = parts.get(2).and_then(|t| {
        let components: Vec<&str> = t.split(':').collect();
        if components.len() == 2 {
            let h = components[0].parse().ok()?;
            let m = components[1].parse().ok()?;
            Some((h, m))
        } else {
            None
        }
    });

    Some((date, time))
}

pub fn find_timestamps<'a>(
    content: &'a str,
    keyword: Option<&str>,
) -> impl Iterator<Item = &'a str> {
    content.lines().flat_map(move |line| {
        let search_in = match keyword {
            Some(kw) => line.find(kw).map(|pos| &line[pos + kw.len()..]),
            None => Some(line),
        };

        search_in.into_iter().flat_map(|text| {
            let mut results = Vec::new();
            let mut pos = 0;
            while let Some(start) = text[pos..].find('<') {
                let actual_start = pos + start;
                if let Some(end) = text[actual_start..].find('>') {
                    results.push(&text[actual_start + 1..actual_start + end]);
                    pos = actual_start + end + 1;
                } else {
                    break;
                }
            }
            results
        })
    })
}

pub fn parse_existing_date(content: &str, input_type: &DateInputType) -> Option<NaiveDate> {
    let keyword = match input_type {
        DateInputType::Plain => return parse_any_date(content),
        DateInputType::Scheduled => Some("SCHEDULED:"),
        DateInputType::Deadline => Some("DEADLINE:"),
    };

    find_timestamps(content, keyword)
        .find_map(|ts| parse_org_timestamp(ts).map(|(date, _)| date))
}

pub fn parse_any_date(content: &str) -> Option<NaiveDate> {
    find_timestamps(content, None)
        .find_map(|ts| parse_org_timestamp(ts).map(|(date, _)| date))
}

pub fn parse_existing_time(content: &str, input_type: &DateInputType) -> (u32, u32) {
    let keyword = match input_type {
        DateInputType::Plain => return parse_any_time(content),
        DateInputType::Scheduled => Some("SCHEDULED:"),
        DateInputType::Deadline => Some("DEADLINE:"),
    };

    find_timestamps(content, keyword)
        .find_map(|ts| parse_org_timestamp(ts).and_then(|(_, time)| time))
        .unwrap_or((0, 0))
}

pub fn parse_any_time(content: &str) -> (u32, u32) {
    find_timestamps(content, None)
        .find_map(|ts| parse_org_timestamp(ts).and_then(|(_, time)| time))
        .unwrap_or((0, 0))
}

pub fn get_display_date(todo: &TodoEntry) -> Option<NaiveDate> {
    let scheduled = parse_existing_date(&todo.content, &DateInputType::Scheduled);
    let deadline = parse_existing_date(&todo.content, &DateInputType::Deadline);
    let plain = parse_any_date(&todo.content);

    let today = Local::now().date_naive();

    let mut dates = Vec::new();
    if let Some(d) = scheduled {
        dates.push(d);
    }
    if let Some(d) = deadline {
        dates.push(d);
    }
    if let Some(d) = plain {
        dates.push(d);
    }

    if dates.is_empty() {
        return None;
    }

    dates
        .into_iter()
        .min_by_key(|date| (*date - today).num_days().abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DateInputType;

    #[test]
    fn test_parse_org_timestamp_date_only() {
        let (date, time) = parse_org_timestamp("2026-03-16 Mon").unwrap();
        assert_eq!(date.to_string(), "2026-03-16");
        assert!(time.is_none());
    }

    #[test]
    fn test_parse_org_timestamp_with_time() {
        let (date, time) = parse_org_timestamp("2026-03-16 Mon 09:30").unwrap();
        assert_eq!(date.to_string(), "2026-03-16");
        assert_eq!(time, Some((9, 30)));
    }

    #[test]
    fn test_parse_org_timestamp_invalid() {
        assert!(parse_org_timestamp("not-a-date").is_none());
    }

    #[test]
    fn test_find_timestamps_no_keyword() {
        let content = "Some text <2026-03-16 Mon> and <2026-03-17 Tue>";
        let results: Vec<_> = find_timestamps(content, None).collect();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("2026-03-16"));
        assert!(results[1].contains("2026-03-17"));
    }

    #[test]
    fn test_find_timestamps_with_keyword() {
        let content = "SCHEDULED: <2026-03-16 Mon>\nDEADLINE: <2026-03-20 Fri>";
        let results: Vec<_> = find_timestamps(content, Some("SCHEDULED:")).collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("2026-03-16"));
    }

    #[test]
    fn test_parse_existing_date_scheduled() {
        let content = "* TODO Task\nSCHEDULED: <2026-03-16 Mon>\n";
        let date = parse_existing_date(content, &DateInputType::Scheduled).unwrap();
        assert_eq!(date.to_string(), "2026-03-16");
    }

    #[test]
    fn test_parse_existing_date_deadline() {
        let content = "* TODO Task\nDEADLINE: <2026-04-01 Wed>\n";
        let date = parse_existing_date(content, &DateInputType::Deadline).unwrap();
        assert_eq!(date.to_string(), "2026-04-01");
    }

    #[test]
    fn test_parse_any_date() {
        let content = "Some note with <2026-05-10 Sun> inline.";
        let date = parse_any_date(content).unwrap();
        assert_eq!(date.to_string(), "2026-05-10");
    }

    #[test]
    fn test_parse_existing_time() {
        let content = "SCHEDULED: <2026-03-16 Mon 14:45>\n";
        let (h, m) = parse_existing_time(content, &DateInputType::Scheduled);
        assert_eq!(h, 14);
        assert_eq!(m, 45);
    }

    #[test]
    fn test_parse_existing_time_missing() {
        let content = "SCHEDULED: <2026-03-16 Mon>\n";
        let (h, m) = parse_existing_time(content, &DateInputType::Scheduled);
        assert_eq!(h, 0);
        assert_eq!(m, 0);
    }
}
