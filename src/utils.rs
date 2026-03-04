use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, Weekday};

use crate::models::TagWithCategory;

pub fn parse_tags(tags_str: &str) -> Vec<TagWithCategory> {
    if tags_str.is_empty() {
        return vec![];
    }

    tags_str
        .split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, "::");
            Some(TagWithCategory {
                category: parts.next()?.trim().to_string(),
                tag: parts.next()?.trim().to_string(),
            })
        })
        .collect()
}

pub fn iso_week_key_from_datetime(transacted_at: &NaiveDateTime) -> String {
    let iso_week = transacted_at.date().iso_week();
    format!("{}-W{:02}", iso_week.year(), iso_week.week())
}

pub fn parse_transacted_at(value: &str) -> Result<NaiveDateTime, ()> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S"))
        .map_err(|_| ())
}

pub fn current_iso_week_key() -> String {
    let today = Local::now().date_naive();
    let iso_week = today.iso_week();
    format!("{}-W{:02}", iso_week.year(), iso_week.week())
}

pub fn normalize_week_key(value: &str) -> Result<String, ()> {
    let (year_str, week_str) = value.split_once("-W").ok_or(())?;
    let year = year_str.parse::<i32>().map_err(|_| ())?;
    let week = week_str.parse::<u32>().map_err(|_| ())?;

    if NaiveDate::from_isoywd_opt(year, week, Weekday::Mon).is_none() {
        return Err(());
    }

    Ok(format!("{}-W{:02}", year, week))
}
