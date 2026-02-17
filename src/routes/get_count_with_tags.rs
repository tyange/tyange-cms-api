use std::sync::Arc;

use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json},
    Error,
};
use sqlx::{Row, query};

use crate::models::{AppState, CountWithTag, CustomResponse, TagsResponse};

#[handler]
pub async fn get_count_with_tags(
    data: Data<&Arc<AppState>>,
) -> Result<Json<CustomResponse<TagsResponse>>, Error> {
    let result = query(
        r#"
        SELECT t.name AS tag, COUNT(*) AS count
        FROM post_tags pt
        JOIN tags t ON pt.tag_id = t.tag_id
        GROUP BY t.name
        ORDER BY count DESC
        "#,
    )
    .fetch_all(&data.db)
    .await;

    match result {
        Ok(db_tags) => {
            if db_tags.len() == 0 {
                return Ok(Json(CustomResponse {
                    status: true,
                    data: Some(TagsResponse {
                        tags: Vec::<CountWithTag>::new(),
                    }),
                    message: Some(String::from("조회된 태그가 하나도 없어요.")),
                }));
            }

            Ok(Json(CustomResponse {
                status: true,
                data: Some(TagsResponse {
                    tags: db_tags
                        .iter()
                        .map(|db_tag| {
                            let tag_response_db = CountWithTag {
                                tag: db_tag.get("tag"),
                                count: db_tag.get("count"),
                            };
                            CountWithTag::from(tag_response_db)
                        })
                        .collect(),
                }),
                message: None,
            }))
        }
        Err(err) => Err(Error::from_string(
            format!("Error fetching tags: {}", err),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
