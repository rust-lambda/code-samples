use lambda_http::http::StatusCode;
use lambda_http::{Error, Response};
use serde::Serialize;

pub fn redirect_response(location: &str) -> Result<Response<String>, Error> {
    let response = Response::builder()
        .status(&StatusCode::FOUND)
        .header("Location", location)
        .body("".to_string())
        .map_err(Box::new)?;

    Ok(response)
}

pub fn empty_response(status: &StatusCode) -> Result<Response<String>, Error> {
    let response = Response::builder()
        .status(status)
        .body("".to_string())
        .map_err(Box::new)?;

    Ok(response)
}

pub fn json_response(
    status: &StatusCode,
    body: &impl Serialize,
) -> Result<Response<String>, Error> {
    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .map_err(Box::new)?;

    Ok(response)
}
