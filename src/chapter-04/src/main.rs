use crate::core::{ShortenUrlRequest, UrlShortener};
use crate::utils::generate_api_response;
use lambda_http::http::StatusCode;
use lambda_http::{
    run, service_fn, tracing, Error, IntoResponse, Request, RequestExt, RequestPayloadExt, Response,
};
use std::env;

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
                None => generate_api_response(400, "".to_string()),
                Some(shorten_url_request) => {
                    let shortened_url_response =
                        url_shortener.shorten_url(shorten_url_request).await;

                    let response = match shortened_url_response {
                        Ok(response) => {
                            generate_api_response(200, serde_json::to_string(&response).unwrap())?
                        }
                        // TODO: handle error better (this should be a 500)
                        Err(_) => generate_api_response(400, "Bad request".to_string())?,
                    };

                    Ok(response)
                }
            }
        }
        "GET" => {
            let short_url = event
                .path_parameters_ref()
                .and_then(|params| params.first("shortUrl"))
                .unwrap_or("");

            let full_url = url_shortener.retrieve_url(short_url.to_string()).await;

            match full_url {
                Err(e) => {
                    tracing::error!("Failed to retrieve URL: {:?}", e);
                    Ok(generate_api_response(
                        500,
                        "Internal Server Error".to_string(),
                    )?)
                }
                Ok(None) => Ok(generate_api_response(404, "".to_string())?),
                Ok(Some(url)) => {
                    let response = Response::builder()
                        .status(StatusCode::from_u16(302).unwrap())
                        .header("Location", url)
                        .body("".to_string())
                        .map_err(Box::new)?;

                    Ok(response)
                }
            }
        }
        _ => generate_api_response(405, "Method not allowed".to_string()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let shortener = UrlShortener::new(table_name, dynamodb_client);

    run(service_fn(|event| function_handler(&shortener, event))).await
}
