use std::env;

use dotenv::dotenv;
use tyange_cms_backend::auth::jwt::Claims;

#[test]
fn test_validate_token() {
    dotenv().ok();

    println!("test");
    println!("test1");

    let secret = env::var("JWT_ACCESS_SECRET").unwrap();
    let claims = Claims::new("test", "access", 15);
    let access_token = claims.to_token(&secret.as_bytes()).unwrap();
    let result = Claims::validate_token(&access_token, &secret.as_bytes()).unwrap();
    assert!(result);
}
