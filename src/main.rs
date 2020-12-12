//! Default Compute@Edge template program.
extern crate log_fastly;
mod spacexTLE;
use fastly::http::{Method, StatusCode};
use fastly::{Body, Error, Request, Response, ResponseExt};
use spacexTLE::spacex_tle;
use serde_json::json;

const N2YO_API_KEY: &str = "YBLNQJ-JUG3KB-XS5BRT-1JX2";

const TXN_LIMIT: i32 = 6;

/// The entry point for your application.
///
/// This function is triggered when your service receives a client request. It could be used to
/// route based on the request properties (such as method or path), send the request to a backend,
/// make completely new requests, and/or generate synthetic responses.
///
/// If `main` returns an error, a 500 error response will be delivered to the client.
#[fastly::main]
fn main(req: Request<Body>) -> Result<impl ResponseExt, Error> {
    log_fastly::init_simple("stdout", log::LevelFilter::Debug);
    log::debug!("*******************************************************");
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
        let mut spacex_tle = spacex_tle::new(N2YO_API_KEY, TXN_LIMIT);
        let payload_tles = spacex_tle.payload_tles(mission_id);
        if payload_tles.is_some() {
            let json = json!(payload_tles);
            return Ok(Response::builder().status(StatusCode::OK).header("Content-Type", "application/json").body(Body::from(format!("{}",json))).unwrap());
        } else { 
           return Ok(Response::builder().status(StatusCode::NOT_FOUND).body(Body::from(format!("No TLEs found for mission {}", mission_id))).unwrap()); 
        } 
    }

    let body = Body::from(format!("Invalid request path: {}", req.uri().path())); 
    let res = Response::builder().status(StatusCode::BAD_REQUEST).body(body).unwrap(); 
    return Ok(res);
}

