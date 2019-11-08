use super::super::*;

use actix_web::{dev::Service, http::StatusCode, test, App};

#[test]
fn register_with_valid_data() {
    dotenv::dotenv().ok();
    dotenv::from_filename(".env.dist").ok();
    let database_url = env::var("DATABASE_URL").unwrap();
    let pool = db::create_connection_pool(database_url.as_str()).unwrap();
    let mut app = test::init_service(App::new().configure(|c| app_config(c, pool)));
    let req = test::TestRequest::get().uri("/").to_request();
    let response = test::block_on(app.call(req)).unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Call to '/' returns 200 OK."
    );

    let payload = user::UserFormInput::new("test@example.com".to_string(), "mypass".to_string());

    let req = test::TestRequest::post()
        .uri("/user/register")
        .set_form(&payload)
        .to_request();

    let response = test::block_on(app.call(req)).unwrap();
    assert_response_ok(&response.response());

    let body = get_response_body(&response.response());
    assert_eq!(
        body.as_str(),
        "Your email is test@example.com with password mypass"
    );
}
