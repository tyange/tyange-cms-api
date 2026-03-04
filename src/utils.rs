use chrono::{Datelike, NaiveDateTime};

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
