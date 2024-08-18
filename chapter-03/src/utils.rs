use lambda_http::http::StatusCode;
use lambda_http::{Error, Response};

pub fn generate_api_response(status: &StatusCode, body: &str) -> Result<Response<String>, Error> {
    let response = Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(body.to_string())
        .map_err(Box::new)?;

    Ok(response)
}
