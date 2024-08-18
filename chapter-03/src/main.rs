use crate::core::{ShortenUrlRequest, UrlShortener};
use crate::utils::generate_api_response;
use http::Method;
use lambda_http::http::StatusCode;
use lambda_http::{
    run, service_fn, tracing, Error, IntoResponse, Request, RequestExt, RequestPayloadExt, Response,
};
mod core;
mod utils;

async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    // Manually writing a router in Lambda is not a best practice, in practice you would either use seperate Lambda functions per endpoint or use a web framework like Actix or Axum inside Lambda.
    // This is purely for demonstration purposes to allow us to build a functioning URL shortener and share memory between GET and POST requests.
    match event.method() {
        &Method::POST => {
            if let Some(shorten_url_request) = event.payload::<ShortenUrlRequest>()? {
                let shortened_url_response = url_shortener.shorten_url(shorten_url_request);
                Ok(generate_api_response(
                    &StatusCode::OK,
                    &serde_json::to_string(&shortened_url_response).unwrap(),
                )?)
            } else {
                generate_api_response(&StatusCode::BAD_REQUEST, "Bad Request")
            }
        }
        &Method::GET => {
            let link_id = event
                .path_parameters_ref()
                .and_then(|params| params.first("linkId"))
                .unwrap_or("");

            if link_id.is_empty() {
                generate_api_response(&StatusCode::NOT_FOUND, "Not Found")
            
            } else if let Some(url) = url_shortener.retrieve_url(link_id) {
                let response = Response::builder()
                    .status(&StatusCode::FOUND)
                    .header("Location", url)
                    .body("".to_string())
                    .map_err(Box::new)?;

                Ok(response)
            
            } else {
                Ok(generate_api_response(&StatusCode::NOT_FOUND, "Not Found")?)
            }

        }
        _ => generate_api_response(&StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let shortener = UrlShortener::new();

    run(service_fn(|event| function_handler(&shortener, event))).await
}
