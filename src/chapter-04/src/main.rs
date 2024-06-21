use crate::core::{ShortenUrlRequest, UrlShortener};
use crate::utils::generate_api_response;
use lambda_http::http::StatusCode;
use lambda_http::{
    run, service_fn, tracing, Error, IntoResponse, Request, RequestExt, RequestPayloadExt, Response,
};
use std::env;
use url_info::UrlInfo;

mod core;
pub mod url_info;
mod utils;

async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);
    match (event.method().as_str(), event.raw_http_path()) {
        ("POST", "/links") => {
            let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

            match shorten_url_request_body {
                None => generate_api_response(400, "Bad request"),
                Some(shorten_url_request) => {
                    let shortened_url_response =
                        url_shortener.shorten_url(shorten_url_request).await;

                    let response = match shortened_url_response {
                        Ok(response) => {
                            generate_api_response(200, &serde_json::to_string(&response).unwrap())?
                        }
                        Err(e) => {
                            tracing::error!("Failed to shorten URL: {:?}", e);
                            generate_api_response(500, "Internal Server Error")?
                        }
                    };

                    Ok(response)
                }
            }
        }
        ("GET", "/links") => {
            let query_params = event.query_string_parameters();
            let last_evaluated_id = query_params.first("last_evaluated_id");

            let links = url_shortener.list_urls(last_evaluated_id).await;
            match links {
                Ok(links) => {
                    let response = Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(serde_json::to_string(&links)?)
                        .map_err(Box::new)?;
                    Ok(response)
                }
                Err(e) => {
                    tracing::error!("Failed to list URLs: {:?}", e);
                    Ok(generate_api_response(500, "Internal Server Error")?)
                }
            }
        }
        ("GET", _) => {
            let link_id = event
                .path_parameters_ref()
                .and_then(|params| params.first("linkId"))
                .unwrap_or("");

            if link_id.is_empty() {
                return generate_api_response(404, "Not Found");
            }

            let full_url = url_shortener.retrieve_url(link_id).await;

            match full_url {
                Err(e) => {
                    tracing::error!("Failed to retrieve URL: {:?}", e);
                    Ok(generate_api_response(500, "Internal Server Error")?)
                }
                Ok(None) => Ok(generate_api_response(404, "Not Found")?),
                Ok(Some(url)) => {
                    if let Err(e) = url_shortener.increment_clicks(link_id).await {
                        tracing::error!("Failed to increment clicks: {:?}", e);
                    }
                    let response = Response::builder()
                        .status(StatusCode::from_u16(302).unwrap())
                        .header("Location", url)
                        .body("".to_string())
                        .map_err(Box::new)?;

                    Ok(response)
                }
            }
        }
        _ => generate_api_response(405, "Method not allowed"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let table_name = env::var("TABLE_NAME").expect("TABLE_NAME is not set");
    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    let url_info = UrlInfo::new(http_client);
    let shortener = UrlShortener::new(&table_name, dynamodb_client, url_info);

    run(service_fn(|event| function_handler(&shortener, event))).await
}
