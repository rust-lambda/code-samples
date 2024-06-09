use crate::core::{ShortenUrlRequest, UrlShortener};
use crate::utils::generate_api_response;
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
    match event.method().as_str() {
        "POST" => {
            let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

            match shorten_url_request_body {
                None => generate_api_response(400, "Bad Request"),
                Some(shorten_url_request) => {
                    let shortened_url_response = url_shortener.shorten_url(shorten_url_request);

                    let response = match shortened_url_response {
                        Ok(response) => {
                            generate_api_response(200, &serde_json::to_string(&response).unwrap())?
                        }
                        Err(_) => generate_api_response(400, "Bad Request")?,
                    };

                    Ok(response)
                }
            }
        }
        "GET" => {
            let link_id = event
                .path_parameters_ref()
                .and_then(|params| params.first("linkId"))
                .unwrap_or("");

            if link_id.is_empty() {
                return generate_api_response(404, "Not Found");
            }

            let full_url = url_shortener.retrieve_url(link_id);

            match full_url {
                None => Ok(generate_api_response(404, "Not Found")?),
                Some(url) => {
                    let response = Response::builder()
                        .status(StatusCode::from_u16(302).unwrap())
                        .header("Location", url)
                        .body("".to_string())
                        .map_err(Box::new)?;

                    Ok(response)
                }
            }
        }
        _ => generate_api_response(405, "Method Not Allowed"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let shortener = UrlShortener::new();

    run(service_fn(|event| function_handler(&shortener, event))).await
}
