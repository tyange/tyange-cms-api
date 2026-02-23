use std::collections::BTreeMap;
use std::sync::Arc;

use poem::{
    Error, handler, http::StatusCode, web::{Data, Json}
};
use sqlx::{Row, query};

use crate::models::{AppState, CustomResponse, TagsWithCategory};

#[handler]
pub async fn get_tags_with_category(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<Vec<TagsWithCategory>>>, Error> {
    let result = query(
        r#"
        SELECT category, name FROM tags ORDER BY category
        "#,
    )
    .fetch_all(&data.db)
    .await;

    match result {
        Ok(db_tags) => {
            if db_tags.is_empty() {
                return Ok(Json(CustomResponse {
                    status: true,
                    data: Some(Vec::<TagsWithCategory>::new()),
                    message: Some(String::from("조회된 태그-카테고리가 하나도 없어요.")),
                }));
            }

            let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for row in &db_tags {
                let category: String = row.get("category");
                let name: String = row.get("name");
                map.entry(category).or_insert_with(Vec::new).push(name);
            }

            let tags_with_category = map
                .into_iter()
                .map(|(category, tags)| TagsWithCategory { category, tags })
                .collect();

            Ok(Json(CustomResponse {
                status: true,
                data: Some(tags_with_category),
                message: None,
            }))
        }
        Err(err) => Err(Error::from_string(
            format!("Error fetching tags: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}