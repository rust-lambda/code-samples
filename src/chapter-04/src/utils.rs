use lambda_http::http::StatusCode;
use lambda_http::{Error, Response};

pub fn generate_api_response(status: u16, body: &str) -> Result<Response<String>, Error> {
    let response = Response::builder()
        .status(StatusCode::from_u16(status).unwrap())
        .header("content-type", "application/json")
        .body(body.to_string())
        .map_err(Box::new)?;

    Ok(response)
}
