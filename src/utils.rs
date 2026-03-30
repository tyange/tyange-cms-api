use chrono::{Datelike, NaiveDateTime};
use serde_json::Value;

use crate::models::{Tag, TagWithCategory};

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

pub fn serialize_tags(tags: &[Tag]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| String::from("[]"))
}

pub fn deserialize_legacy_post_tags(value: &str) -> Vec<Tag> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Ok(parsed) = serde_json::from_str::<Vec<Tag>>(trimmed) {
        return parsed
            .into_iter()
            .filter(|tag| !tag.tag.trim().is_empty())
            .collect();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if let Some(items) = parsed.as_array() {
            let extracted = items
                .iter()
                .filter_map(|item| {
                    if let Some(tag) = item.get("tag").and_then(Value::as_str) {
                        let category = item
                            .get("category")
                            .and_then(Value::as_str)
                            .unwrap_or("general");
                        return Some(Tag {
                            tag: tag.trim().to_string(),
                            category: category.trim().to_string(),
                        });
                    }

                    item.as_str().map(|tag| Tag {
                        tag: tag.trim().to_string(),
                        category: String::from("general"),
                    })
                })
                .filter(|tag| !tag.tag.is_empty())
                .collect::<Vec<_>>();

            if !extracted.is_empty() {
                return extracted;
            }
        }
    }

    parse_tags(trimmed)
        .into_iter()
        .map(|tag| Tag {
            tag: tag.tag,
            category: tag.category,
        })
        .collect()
}
