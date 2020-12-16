mod spacex_tle;

use config::{Config, FileFormat};
use fastly::http::{Method, StatusCode};
use fastly::{Body, Error, Request, Response, ResponseExt};
use serde_json::json;
use spacex_tle::SpacexTLE;

const LOGGING_ENDPOINT: &str = "Syslog";
const N2YO_API_KEY: &str = "YBLNQJ-JUG3KB-XS5BRT-1JX2";
const TXN_LIMIT: i64 = 6;

/// The entry point for your application.
///
/// This function is triggered when your service receives a client request. It could be used to
/// route based on the request properties (such as method or path), send the request to a backend,
/// make completely new requests, and/or generate synthetic responses.
///
/// If `main` returns an error, a 500 error response will be delivered to the client.
#[fastly::main]
fn main(req: Request<Body>) -> Result<Response<Body>, Error> {
    logging_init();
    log::debug!("Request: {} {}", req.method(), req.uri());

    // We can filter requests that have unexpected methods.
    const VALID_METHODS: [Method; 1] = [Method::GET];
    if !(VALID_METHODS.contains(req.method())) {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::from("This method is not allowed"))?);
    }

    let url_parts = &req.uri().path().split("/").collect::<Vec<&str>>()[1..];
    if url_parts[0] == "tle" {
        let mission_id = url_parts[1];
        let mut spacex_tle = SpacexTLE::new(N2YO_API_KEY, TXN_LIMIT);
        let payload_tles = spacex_tle.payload_tles(mission_id)?;
        if payload_tles.is_some() {
            let json = json!(payload_tles);
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(format!("{}", json)))
                .unwrap());
        } else {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!(
                    "No TLEs found for mission {}",
                    mission_id
                )))
                .unwrap());
        }
    }

    let body = Body::from(format!("Invalid request path: {}", req.uri().path()));
    let res = Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(body)
        .unwrap();
    return Ok(res);
}

/// This function reads the fastly.toml file and gets the deployed version. This is only run at
/// compile time. Since we bump the version number after building (during the deploy) we return
/// the version incremented by one so the version returned will match the deployed version.
/// courtesy Jim Rainville <jrainville@fastly.com>
fn get_version() -> i32 {
    Config::new()
        .merge(config::File::from_str(
            include_str!("../fastly.toml"), // assumes the existence of fastly.toml
            FileFormat::Toml,
        ))
        .unwrap()
        .get_str("version")
        .unwrap()
        .parse::<i32>()
        .unwrap_or(0)
        + 1
}

/// initialize the logger
/// courtesy Jim Rainville <jrainville@fastly.com>
fn logging_init() {
    log_fastly::Logger::builder()
        .max_level(log::LevelFilter::Debug)
        .default_endpoint(LOGGING_ENDPOINT)
        .init();
    fastly::log::set_panic_endpoint(LOGGING_ENDPOINT).unwrap();
    log::debug!("*******************************************************");
    log::debug!("Get Test Version:{}", get_version());
}
