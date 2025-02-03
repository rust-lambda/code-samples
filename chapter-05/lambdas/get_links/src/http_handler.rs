use lambda_http::RequestExt;
use lambda_http::{http::StatusCode, tracing, Error, IntoResponse, Request};
use shared::core::UrlShortener;
use shared::response::{empty_response, json_response};

pub(crate) async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    tracing::info!("Received event: {:?}", event);

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
