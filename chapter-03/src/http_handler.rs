use crate::core::{ShortenUrlRequest, UrlShortener};
use crate::utils::{empty_response, json_response, redirect_response};
use lambda_http::{
    http::{Method, StatusCode},
    Error, IntoResponse, Request, RequestExt, RequestPayloadExt,
};

pub(crate) async fn function_handler(
    url_shortener: &UrlShortener,
    event: Request,
) -> Result<impl IntoResponse, Error> {
    // Manually writing a router in Lambda is not a best practice, in practice you would either use seperate Lambda functions per endpoint or use a web framework like Actix or Axum inside Lambda.
    // This is purely for demonstration purposes to allow us to build a functioning URL shortener and share memory between GET and POST requests.
    match *event.method() {
        Method::POST => {
            if let Some(shorten_url_request) = event.payload::<ShortenUrlRequest>()? {
                let shortened_url_response = url_shortener.shorten_url(shorten_url_request);
                json_response(&StatusCode::OK, &shortened_url_response)
            } else {
                empty_response(&StatusCode::BAD_REQUEST)
            }
        }

        Method::GET => {
            let link_id = event
                .path_parameters_ref()
                .and_then(|params| params.first("linkId"))
                .unwrap_or("");

            if link_id.is_empty() {
                empty_response(&StatusCode::NOT_FOUND)
            } else if let Some(url) = url_shortener.retrieve_url(link_id) {
                redirect_response(&url)
            } else {
                Ok(empty_response(&StatusCode::NOT_FOUND)?)
            }
        }

        _ => empty_response(&StatusCode::METHOD_NOT_ALLOWED),
    }
}
