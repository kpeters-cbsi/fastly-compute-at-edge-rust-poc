use fastly::http::{HeaderValue, Method};
use fastly::request::CacheOverride;
use fastly::{Body, Error, Request, RequestExt, Response};
use serde_json::value;
use std::collections::HashMap;
use std::time::Instant;

const BACKEND_SPACEXDATA: &str = "SpaceXData";
const BACKEND_N2YO: &str = "n2yo";
const SPACEXDATA_URI: &str = "https://api.spacexdata.com/v3/";
const N2YO_URI: &str = "https://api.n2yo.com/rest/v1/satellite/";

#[derive(Debug)]
pub struct SpacexTLE {
    n2yo_api_key: &'static str,
    txn_limit: i64,
    _txn_count: i64,
}

impl SpacexTLE {
    pub fn new(n2yo_api_key: &'static str, txn_limit: i64) -> SpacexTLE {
        return SpacexTLE {
            n2yo_api_key,
            txn_limit,
            _txn_count: 0,
        };
    }

    pub fn payload_tles(
        &mut self,
        mission_id: &str,
    ) -> Result<Option<HashMap<String, Vec<String>>>, Error> {
        let t0 = Instant::now();
        let norad_ids = if let Some(val) = self.get_norad_ids_for_mission(mission_id)? {
            val
        } else {
            return Ok(None);
        };

        self.log_elapsed(t0, "get NORAD IDs");

        let mut payload_tles = HashMap::new();
        for (payload_id, norad_ids_for_payload) in norad_ids {
            let t0 = Instant::now();
            let limit = (self.txn_limit - self.txn_count()) as usize;
            if limit > 0 {
                let slice_end = limit.min(norad_ids_for_payload.len());
                let slice = &norad_ids_for_payload[..slice_end];
                log::debug!("Get TLEs for NORAD IDs {:?}", &slice);
                let tles = self.get_tles_for_norad_ids(slice)?;
                self.log_elapsed(t0, "Get TLEs for NORAD IDs");
                payload_tles.insert(payload_id.to_owned(), tles);
            } else {
                log::info!(
                    "TXN limit ({}) reached. Skip TLEs for payload \"{}\"",
                    self.txn_limit,
                    payload_id
                );
            }
        }
        return Ok(Some(payload_tles));
    }

    fn txn_count(&self) -> i64 {
        log::debug!("TXN count: {}", self._txn_count);
        return self._txn_count;
    }

    fn log_elapsed(&self, instant: Instant, message: &str) {
        let elapsed = instant.elapsed();
        log::info!("Elapsed in {}: {:#?}", message, elapsed)
    }

    fn get_norad_ids_for_mission(
        &mut self,
        mission_id: &str,
    ) -> Result<Option<HashMap<String, Vec<i64>>>, Error> {
        log::debug!("Request NORAD IDs for mission \"{}\"", mission_id);
        let mut filter = HashMap::new();
        filter.insert("mission_id", mission_id);
        filter.insert(
            "filter",
            "rocket/second_stage/payloads/(payload_id,norad_id)",
        );
        let response = self.spacexdata_request("launches", filter)?;
        if response.is_array() {
            let launches = response.as_array().unwrap();
            if launches.len() == 0 {
                log::debug!("No launches found for mission {}", mission_id);
                return Ok(None);
            } else {
                log::debug!(
                    "{} launches found for mission {}",
                    launches.len(),
                    mission_id
                );
                let mut payload_norad_ids = HashMap::new();
                let mut count = 0;
                for launch in launches {
                    count = count + 1;
                    let rocket = launch.get("rocket").unwrap();
                    let second_stage = rocket.get("second_stage").unwrap();
                    let payloads = second_stage.get("payloads").unwrap().as_array().unwrap();
                    log::debug!(
                        "{} payloads in mission {}, launch {}",
                        payloads.len(),
                        mission_id,
                        count
                    );
                    for payload in payloads {
                        let payload_id = payload
                            .get("payload_id")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_owned();
                        let norad_ids = payload.get("norad_id").unwrap().as_array().unwrap();
                        log::debug!("Payload {} has {} NORAD ID(s)", payload_id, norad_ids.len());
                        let norad_ids = norad_ids.iter().map(|x| x.as_i64().unwrap()).collect();
                        payload_norad_ids.insert(payload_id, norad_ids);
                    }
                }
                return Ok(Some(payload_norad_ids));
            }
        } else {
            panic!("SpaceXData API did not return an array")
        }
    }

    fn default_header_value(&self) -> HeaderValue {
        return HeaderValue::from_str("").unwrap();
    }

    fn get_tles_for_norad_ids(&mut self, norad_ids: &[i64]) -> Result<Vec<String>, Error> {
        let mut tles = Vec::new();
        for norad_id in norad_ids {
            let path = format!("tle/{}", norad_id);
            let response = self.n2yo_request(&path)?;
            let tle_str = response.get("tle").unwrap().as_str().unwrap();
            if !tle_str.is_empty() {
                let split: Vec<String> = tle_str.split("\r\n").map(String::from).collect();
                for tle in split {
                    tles.push(tle);
                }
            }
        }
        return Ok(tles);
    }

    fn spacexdata_request(
        &mut self,
        path: &str,
        params: HashMap<&str, &str>,
    ) -> Result<value::Value, Error> {
        let mut uri: String = SPACEXDATA_URI.to_owned();
        uri.push_str(path);
        if params.len() > 0 {
            uri.push_str("?");
            let mut count = 0;
            for (key, value) in params {
                if count > 0 {
                    uri.push('&');
                }
                let kv = format!("{}={}", key, value);
                uri.push_str(&kv);
                count = count + 1;
            }
        }
        log::debug!("SpaceXData URI: {}", uri);
        let response = self.request(Method::GET, &uri, &BACKEND_SPACEXDATA, None, None, None)?;
        let default_header_value = self.default_header_value();
        let content_type = response
            .headers()
            .get("Content-Type")
            .unwrap_or(&default_header_value)
            .to_str()
            .unwrap();
        if content_type.contains("json") {
            let body_str = &response.into_body().into_string();
            return Ok(serde_json::from_str(&body_str).unwrap());
        } else {
            panic!("SpaceXData API did not return JSON");
        }
    }

    fn n2yo_request(&mut self, path: &str) -> Result<value::Value, Error> {
        let mut uri: String = N2YO_URI.to_owned();
        uri.push_str(path);
        uri.push_str("?apiKey=");
        uri.push_str(self.n2yo_api_key);
        let response = self.request(Method::GET, &uri, &BACKEND_N2YO, None, None, None)?;
        let default_header_value = self.default_header_value();
        let content_type = response
            .headers()
            .get("Content-Type")
            .unwrap_or(&default_header_value)
            .to_str()
            .unwrap();
        if content_type.contains("json") {
            let body_str = response.into_body().into_string().to_owned();
            return Ok(serde_json::from_str(&body_str).unwrap());
        } else {
            panic!("n2yo API did not return JSON");
        }
    }

    fn request(
        &mut self,
        method: Method,
        uri: &str,
        backend: &str,
        headers: Option<HashMap<String, String>>,
        body: Option<&str>,
        cache_override: Option<CacheOverride>,
    ) -> Result<Response<Body>, Error> {
        let t0 = Instant::now();
        log::debug!("[TXN {}] {} {}", self.txn_count(), method, uri);
        let mut builder = Request::builder().method(method.as_str()).uri(uri);

        // Set request headers if passed
        if headers.is_some() {
            for (key, value) in &headers.unwrap() {
                builder = builder.header(key, value);
            }
        }

        // body is the "terminal state" of the builder, so we have to call body().unwrap to get the Request
        // object
        let mut request;
        match body {
            Some(body) => request = builder.body(Body::from(body)).unwrap(),
            None => request = builder.body(Body::from("")).unwrap(),
        };

        cache_override.and_then(|c| Some(*request.cache_override_mut() = c));
        let response = request.send(backend)?;
        log::debug!("Request complete");

        let (parts, body) = response.into_parts();
        let body_str = body.into_string();
        log::debug!("Response body: {}", body_str);
        let new_response = Response::from_parts(parts, Body::from(body_str));
        let log_str = format!("({}) {} {}", backend, method.as_str(), uri);
        self.log_elapsed(t0, &log_str);
        self.increment_txn_count();
        return Ok(new_response);
    }

    fn increment_txn_count(&mut self) {
        self._txn_count = self._txn_count + 1;
    }
}
