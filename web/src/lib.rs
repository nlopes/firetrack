#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate tera;

#[cfg(test)]
mod firetrack_test;
#[cfg(test)]
mod integration_tests;

#[cfg(test)]
use crate::firetrack_test::*;
#[cfg(test)]
use actix_web::test;

mod user;

use actix_files;
use actix_web::{error, middleware, web, App, Error, HttpResponse, HttpServer};
use std::env;
use std::process::exit;

/// A trait that defines functions that will log an error and exit with an error code.
/// These can be used instead of panics to have clean logging in the console.
pub trait ExitWithError<T> {
    /// Unwraps an option or result, yielding the content of a [`Some`] or [`Ok`].
    ///
    /// # Exits
    ///
    /// Logs an error using the text provided by `msg` if the value is a [`None`] or [`Err`] and
    /// exits with an error code.
    fn expect_or_exit(self, msg: &str) -> T;

    /// Unwraps an option or result, yielding the content of a [`Some`] or [`Ok`].
    ///
    /// # Exits
    ///
    /// Exits with an error code if the value is a [`None`] or [`Err`]. If the value is an [`Err`]
    /// the corresponding error message will be logged.
    fn unwrap_or_exit(self) -> T;
}

impl<T> ExitWithError<T> for Option<T> {
    fn expect_or_exit(self, msg: &str) -> T {
        match self {
            Some(val) => val,
            None => {
                error!("{}", msg);
                exit(1);
            }
        }
    }

    fn unwrap_or_exit(self) -> T {
        match self {
            Some(val) => val,
            None => {
                error!("called `Option::unwrap()` on a `None` value");
                exit(1);
            }
        }
    }
}

impl<T, E: std::fmt::Display> ExitWithError<T> for Result<T, E> {
    fn expect_or_exit(self, msg: &str) -> T {
        match self {
            Ok(t) => t,
            Err(_) => {
                error!("{}", msg);
                exit(1);
            }
        }
    }

    fn unwrap_or_exit(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => {
                error!("{}", &e);
                exit(1);
            }
        }
    }
}

// Starts the web server on the given host address and port.
pub fn serve(host: &str, port: &str) {
    // Configure the application.
    let app = || {
        App::new()
            .wrap(middleware::Logger::default())
            .configure(app_config)
    };

    // Start the web server.
    let addr = format!("{}:{}", host, port);
    match HttpServer::new(app).bind(addr) {
        Ok(server) => {
            server.run().unwrap();
        }
        Err(e) => {
            error!("Failed to start web server on {}:{}", host, port);
            error!("{}", e.to_string());
            exit(1);
        }
    }
}

// Controller for the homepage.
fn index(template: web::Data<tera::Tera>) -> Result<HttpResponse, Error> {
    let mut context = tera::Context::new();
    context.insert("title", &"Home");
    let content = template
        .render("index.html", &context)
        .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(content))
}

// Unit tests for the homepage.
#[test]
fn test_index() {
    dotenv::dotenv().ok();

    // Wrap the Tera struct in a HttpRequest and then retrieve it from the request as a Data struct.
    let tera = compile_templates();
    let request = test::TestRequest::get().data(tera).to_http_request();
    let app_data = request.get_app_data().unwrap();

    // Pass the Data struct containing the Tera templates to the index() function. This mimics how
    // actix-web passes the data to the controller.
    let controller = index(app_data);
    let response = test::block_on(controller).unwrap();
    let body = get_response_body(&response);

    assert_response_ok(&response);
    assert_header_title(&body, "Home");
    assert_page_title(&body, "Home");
    assert_navbar(&body);
}

// Configure the application.
fn app_config(config: &mut web::ServiceConfig) {
    let tera = compile_templates();
    config.service(
        web::scope("")
            .data(tera)
            .service(actix_files::Files::new("/css", "static/css"))
            .service(actix_files::Files::new("/images", "static/images"))
            .service(actix_files::Files::new("/js", "static/js"))
            .route("/", web::get().to(index))
            .route("/user/login", web::get().to(user::login_handler))
            .route("/user/register", web::get().to(user::register_handler))
            .route("/user/register", web::post().to(user::register_submit)),
    );
}

// Compile the Tera templates.
fn compile_templates() -> tera::Tera {
    // Determine the path to the templates folder. This depends on whether we are running from the
    // root of the application (e.g. when launched using `cargo run`) or from the library folder
    // (e.g. when running tests).
    let path = if env::current_dir().unwrap().ends_with("web") {
        "templates/**/*"
    } else {
        "web/templates/**/*"
    };
    compile_templates!(path)
}