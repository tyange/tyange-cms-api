pub mod add_user;
pub mod analyze_card_excel;
pub mod create_api_key;
pub mod create_budget_plan;
pub mod create_spending;
pub mod delete_api_key;
pub mod delete_post;
pub mod delete_spending;
pub mod get_all_posts;
pub mod get_api_keys;
pub mod get_budget;
pub mod get_count_with_tags;
pub mod get_portfolio;
pub mod get_post;
pub mod get_posts;
pub mod get_posts_with_tags;
pub mod get_spending;
pub mod get_tags_with_category;
pub mod import_spending_excel;
pub mod login;
pub mod me;
pub mod signup;
pub mod update_active_budget;
pub mod update_portfolio;
pub mod update_post;
pub mod update_spending;
pub mod upload_image;
pub mod upload_post;

#[cfg(test)]
mod budget_spending_scope_test;
#[cfg(test)]
mod post_authorization_test;
#[cfg(test)]
mod signup_test;
