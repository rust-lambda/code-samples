use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request, RequestPayloadExt};
use shared::core::{ShortenUrlRequest, UrlShortener};
use shared::response::{empty_response, json_response};

pub(crate) async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

    let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

    match shorten_url_request_body {
        None => empty_response(&StatusCode::BAD_REQUEST),
        Some(shorten_url_request) => {
            let shortened_url_response = url_shortener.shorten_url(shorten_url_request).await;

            match shortened_url_response {
                Ok(response) => json_response(&StatusCode::OK, &response),
                Err(e) => {
                    tracing::error!("Failed to shorten URL: {:?}", e);
                    empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
    }
}
