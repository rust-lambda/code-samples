use crate::core::{ShortenUrlRequest, UrlShortener};
use crate::utils::{empty_response, json_response, redirect_response};
use lambda_http::{
    http::{Method, StatusCode},
    tracing, Error, IntoResponse, Request, RequestExt, RequestPayloadExt,
};

pub(crate) async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);
    match (event.method(), event.raw_http_path()) {
        (&Method::POST, "/links") => {
            let shorten_url_request_body = event.payload::<ShortenUrlRequest>()?;

            match shorten_url_request_body {
                None => empty_response(&StatusCode::BAD_REQUEST),
                Some(shorten_url_request) => {
                    let shortened_url_response =
                        url_shortener.shorten_url(shorten_url_request).await;

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
        (&Method::GET, "/links") => {
            let query_params = event.query_string_parameters();
            let last_evaluated_id = query_params.first("last_evaluated_id");

            let links = url_shortener.list_urls(last_evaluated_id).await;
            match links {
                Ok(links) => json_response(&StatusCode::OK, &links),
                Err(e) => {
                    tracing::error!("Failed to list URLs: {:?}", e);
                    empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        (&Method::GET, _) => {
            let link_id = event
                .path_parameters_ref()
                .and_then(|params| params.first("linkId"))
                .unwrap_or("");

            if link_id.is_empty() {
                return empty_response(&StatusCode::NOT_FOUND);
            }

            let full_url = url_shortener
                .retrieve_url_and_increment_clicks(link_id)
                .await;

            match full_url {
                Err(e) => {
                    tracing::error!("Failed to retrieve URL: {:?}", e);
                    empty_response(&StatusCode::INTERNAL_SERVER_ERROR)
                }
                Ok(None) => empty_response(&StatusCode::NOT_FOUND),
                Ok(Some(url)) => redirect_response(&url),
            }
        }
        _ => empty_response(&StatusCode::METHOD_NOT_ALLOWED),
    }
}
