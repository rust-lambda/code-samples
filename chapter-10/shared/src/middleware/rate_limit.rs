//! Per-IP rate-limiting middleware for Rust Lambda HTTP handlers.
//!
//! A fixed-window counter backed by DynamoDB. The primary key for each
//! counter row is `"{ip}#{window_bucket}"` where `window_bucket = now / window_duration`.
//! Rows carry a TTL so DynamoDB cleans them up for us.
//!
//! Response headers follow the GitHub / Twitter convention, with
//! `Reset` carrying a Unix epoch second:
//!
//! * `X-RateLimit-Limit`
//! * `X-RateLimit-Remaining`
//! * `X-RateLimit-Reset`
//!
//! On breach we return a plain JSON `429` with `Retry-After`. When the
//! counter store is unreachable we fail closed with a plain JSON `503`.
//! Both responses are tweakable via `RateLimitLayer::on_over_limit` and
//! `RateLimitLayer::on_unavailable`: the override callbacks receive the
//! incoming request plus a pre-built default response, so callers only
//! need to adjust the bits they care about.

use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aws_sdk_dynamodb::types::AttributeValue;
use lambda_http::http::{HeaderValue, Request, Response};
use lambda_http::tower::{Layer, Service};
use lambda_http::{tracing, Body, IntoResponse};
use serde::Serialize;

use crate::middleware::ip_extractor::extract_ip;

/// Information passed to a custom over-limit response builder.
#[derive(Clone, Debug)]
pub struct OverLimitCtx {
    pub ip: IpAddr,
    pub limit: u32,
    pub reset_at: u64,
    pub retry_after: u64,
}

/// Closure that customises the over-limit (HTTP 429) response.
///
/// It receives the incoming request, a **pre-built** default 429 response,
/// and the over-limit context. Return the response as-is to keep the
/// default behaviour, or tweak/replace it before returning.
pub type OverLimitFn =
    Arc<dyn Fn(&Request<Body>, Response<Body>, &OverLimitCtx) -> Response<Body> + Send + Sync>;

/// Closure that customises the counter-store-unavailable (HTTP 503) response.
///
/// Mirrors [`OverLimitFn`]: receives the request and a pre-built default
/// 503 response.
pub type UnavailableFn =
    Arc<dyn Fn(&Request<Body>, Response<Body>) -> Response<Body> + Send + Sync>;

/// Static, cheap-to-clone configuration for [`RateLimitLayer`].
#[derive(Clone)]
pub struct RateLimitConfig {
    pub table_name: String,
    pub max_requests: u32,
    pub window_duration: Duration,
}

/// Tower [`Layer`] that enforces a per-IP fixed-window rate limit.
#[derive(Clone)]
pub struct RateLimitLayer {
    config: Arc<RateLimitConfig>,
    client: aws_sdk_dynamodb::Client,
    over_limit: OverLimitFn,
    unavailable: UnavailableFn,
}

impl RateLimitLayer {
    pub fn new(config: RateLimitConfig, client: aws_sdk_dynamodb::Client) -> Self {
        Self {
            config: Arc::new(config),
            client,
            over_limit: Arc::new(default_over_limit),
            unavailable: Arc::new(default_unavailable),
        }
    }

    /// Override the response returned when a client is over the limit.
    ///
    /// The closure receives the incoming request, a pre-populated 429
    /// response with the standard `RateLimit-*` and `Retry-After` headers
    /// already set, and the [`OverLimitCtx`]. Mutate or replace as needed.
    pub fn on_over_limit<F>(mut self, f: F) -> Self
    where
        F: Fn(&Request<Body>, Response<Body>, &OverLimitCtx) -> Response<Body>
            + Send
            + Sync
            + 'static,
    {
        self.over_limit = Arc::new(f);
        self
    }

    /// Override the response returned when the counter store is unreachable.
    ///
    /// The closure receives the incoming request and a pre-populated 503
    /// response. Mutate or replace as needed.
    pub fn on_unavailable<F>(mut self, f: F) -> Self
    where
        F: Fn(&Request<Body>, Response<Body>) -> Response<Body> + Send + Sync + 'static,
    {
        self.unavailable = Arc::new(f);
        self
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        let store: Arc<dyn RateLimitStore> = Arc::new(DynamoDbRateLimitStore {
            client: self.client.clone(),
        });
        RateLimitService {
            inner,
            store,
            config: Arc::clone(&self.config),
            over_limit: Arc::clone(&self.over_limit),
            unavailable: Arc::clone(&self.unavailable),
        }
    }
}

/// Tower [`Service`] that wraps an inner service, checks the counter on every
/// request, and either short-circuits with a 429 / 503 or forwards to the
/// inner service and stamps the response with `RateLimit-*` headers.
pub struct RateLimitService<S> {
    inner: S,
    store: Arc<dyn RateLimitStore>,
    config: Arc<RateLimitConfig>,
    over_limit: OverLimitFn,
    unavailable: UnavailableFn,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>> + Send + 'static,
    S::Response: IntoResponse + Send,
    S::Future: Send,
    S::Error: Send,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let config = Arc::clone(&self.config);
        let store = Arc::clone(&self.store);
        let over_limit = Arc::clone(&self.over_limit);
        let unavailable = Arc::clone(&self.unavailable);

        // Extract the IP up front, before `inner.call(request)` consumes the
        // request by move.
        let ip = extract_ip(&request);

        // Clone the request so the bail-out callbacks can still see it after
        // we have handed the original off to the inner service.
        let request_for_bailout = request.clone();

        let inner_future = self.inner.call(request);

        Box::pin(async move {
            let Some(ip) = ip else {
                // No identifiable client: fail open so we don't lock out
                // legitimate traffic when a misbehaving proxy drops headers.
                tracing::warn!("rate_limit: could not determine client IP, allowing request");
                let response = inner_future.await?.into_response().await;
                return Ok(response);
            };

            let now = current_epoch_secs();
            // Clamp the window to at least one second; a zero-length window
            // would divide by zero when computing the bucket below.
            let window = config.window_duration.as_secs().max(1);
            let bucket = now / window;
            let reset_at = bucket.saturating_add(1).saturating_mul(window);
            let seconds_until_reset = reset_at.saturating_sub(now);

            let pk = format!("{ip}#{bucket}");
            // Keep the row for one extra window so late-arriving requests in
            // the same bucket still observe a consistent counter.
            let ttl = reset_at.saturating_add(window);

            let count = match store.increment_and_get(&config.table_name, &pk, ttl).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(ip = %ip, window = bucket, error = %e, "rate_limit: DynamoDB error");
                    // Fail closed: build the default 503, then let the
                    // configurable hook tweak it before sending.
                    let pre_built = build_unavailable_response();
                    return Ok(unavailable(&request_for_bailout, pre_built));
                }
            };

            let limit = config.max_requests;
            tracing::debug!(ip = %ip, window = bucket, count, limit, "rate_limit decision");
            if count > limit {
                let ctx = OverLimitCtx {
                    ip,
                    limit,
                    reset_at,
                    retry_after: seconds_until_reset,
                };
                let pre_built = build_over_limit_response(&ctx);
                return Ok(over_limit(&request_for_bailout, pre_built, &ctx));
            }

            let mut response = inner_future.await?.into_response().await;
            let remaining = limit.saturating_sub(count);
            append_rate_limit_headers(response.headers_mut(), limit, remaining, reset_at);
            Ok(response)
        })
    }
}

fn current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn append_rate_limit_headers(
    headers: &mut lambda_http::http::HeaderMap,
    limit: u32,
    remaining: u32,
    reset_at: u64,
) {
    if let Ok(v) = HeaderValue::from_str(&limit.to_string()) {
        headers.insert("X-RateLimit-Limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&remaining.to_string()) {
        headers.insert("X-RateLimit-Remaining", v);
    }
    if let Ok(v) = HeaderValue::from_str(&reset_at.to_string()) {
        headers.insert("X-RateLimit-Reset", v);
    }
}

#[derive(Serialize)]
struct RateLimitErrorBody<'a> {
    error: &'a str,
    retry_after: u64,
}

fn build_over_limit_response(ctx: &OverLimitCtx) -> Response<Body> {
    let body = serde_json::to_string(&RateLimitErrorBody {
        error: "rate limit exceeded",
        retry_after: ctx.retry_after,
    })
    .unwrap_or_else(|_| r#"{"error":"rate limit exceeded"}"#.to_string());

    Response::builder()
        .status(429)
        .header("content-type", "application/json")
        .header("Retry-After", ctx.retry_after.to_string())
        .header("X-RateLimit-Limit", ctx.limit.to_string())
        .header("X-RateLimit-Remaining", "0")
        .header("X-RateLimit-Reset", ctx.reset_at.to_string())
        .body(body.into())
        .expect("valid 429 response")
}

fn build_unavailable_response() -> Response<Body> {
    Response::builder()
        .status(503)
        .header("content-type", "application/json")
        .body(r#"{"error":"service unavailable"}"#.into())
        .expect("valid 503 response")
}

/// Default `on_over_limit` callback: pass the pre-built 429 through unchanged.
fn default_over_limit(
    _request: &Request<Body>,
    response: Response<Body>,
    _ctx: &OverLimitCtx,
) -> Response<Body> {
    response
}

/// Default `on_unavailable` callback: pass the pre-built 503 through unchanged.
fn default_unavailable(_request: &Request<Body>, response: Response<Body>) -> Response<Body> {
    response
}

// ---------------------------------------------------------------------------
// Store abstraction. A trait here makes the service trivially unit-testable
// with an in-memory mock, without needing a local DynamoDB.
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
trait RateLimitStore: Send + Sync {
    async fn increment_and_get(
        &self,
        table_name: &str,
        pk: &str,
        ttl: u64,
    ) -> Result<u32, RateLimitStoreError>;
}

#[derive(Debug, thiserror::Error)]
enum RateLimitStoreError {
    #[error("DynamoDB error: {0}")]
    DynamoDb(String),
}

struct DynamoDbRateLimitStore {
    client: aws_sdk_dynamodb::Client,
}

#[async_trait::async_trait]
impl RateLimitStore for DynamoDbRateLimitStore {
    async fn increment_and_get(
        &self,
        table_name: &str,
        pk: &str,
        ttl: u64,
    ) -> Result<u32, RateLimitStoreError> {
        let result = self
            .client
            .update_item()
            .table_name(table_name)
            .key("pk", AttributeValue::S(pk.to_string()))
            .update_expression("ADD #calls :one SET #ttl = if_not_exists(#ttl, :ttl_val)")
            .expression_attribute_names("#calls", "calls")
            .expression_attribute_names("#ttl", "ttl")
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .expression_attribute_values(":ttl_val", AttributeValue::N(ttl.to_string()))
            .return_values(aws_sdk_dynamodb::types::ReturnValue::UpdatedNew)
            .send()
            .await
            .map_err(|e| RateLimitStoreError::DynamoDb(e.to_string()))?;

        let count = result
            .attributes()
            .and_then(|attrs| attrs.get("calls"))
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<u32>().ok())
            .ok_or_else(|| {
                RateLimitStoreError::DynamoDb(
                    "missing or invalid 'calls' attribute in UpdateItem response".to_string(),
                )
            })?;

        Ok(count)
    }
}

#[cfg(test)]
impl<S> RateLimitService<S> {
    fn with_store(inner: S, store: Arc<dyn RateLimitStore>, config: Arc<RateLimitConfig>) -> Self {
        Self {
            inner,
            store,
            config,
            over_limit: Arc::new(default_over_limit),
            unavailable: Arc::new(default_unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_http::aws_lambda_events::apigw::{
        ApiGatewayV2httpRequestContext, ApiGatewayV2httpRequestContextHttpDescription,
    };
    use lambda_http::http::StatusCode;
    use lambda_http::request::RequestContext;
    use lambda_http::tower::ServiceExt;
    use lambda_http::RequestExt;
    use std::convert::Infallible;
    use std::sync::Mutex;

    struct MockRateLimitStore {
        counters: Mutex<std::collections::HashMap<String, u32>>,
        fail: bool,
    }

    impl MockRateLimitStore {
        fn new() -> Self {
            Self {
                counters: Mutex::new(std::collections::HashMap::new()),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                counters: Mutex::new(std::collections::HashMap::new()),
                fail: true,
            }
        }
    }

    #[async_trait::async_trait]
    impl RateLimitStore for MockRateLimitStore {
        async fn increment_and_get(
            &self,
            _table_name: &str,
            pk: &str,
            _ttl: u64,
        ) -> Result<u32, RateLimitStoreError> {
            if self.fail {
                return Err(RateLimitStoreError::DynamoDb("mock failure".to_string()));
            }
            let mut counters = self.counters.lock().unwrap();
            let count = counters.entry(pk.to_string()).or_insert(0);
            *count += 1;
            Ok(*count)
        }
    }

    fn test_config(max: u32) -> Arc<RateLimitConfig> {
        Arc::new(RateLimitConfig {
            table_name: "test-table".to_string(),
            max_requests: max,
            window_duration: Duration::from_secs(60),
        })
    }

    fn request_with_ip(ip: &str) -> Request<Body> {
        // extract_ip reads the source IP from the API Gateway
        // request context, so the test request needs a populated
        // ApiGatewayV2 context.
        let mut http = ApiGatewayV2httpRequestContextHttpDescription::default();
        http.source_ip = Some(ip.to_string());
        let mut ctx = ApiGatewayV2httpRequestContext::default();
        ctx.http = http;

        let request = Request::builder()
            .method("GET")
            .uri("http://example.com/test")
            .body(Body::Empty)
            .unwrap();
        request.with_request_context(RequestContext::ApiGatewayV2(ctx))
    }

    async fn ok_handler(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"status":"ok"}"#))
            .unwrap())
    }

    #[tokio::test]
    async fn under_limit_calls_inner_service() {
        let config = test_config(10);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::new());
        let service =
            RateLimitService::with_store(lambda_http::tower::service_fn(ok_handler), store, config);

        let response = service
            .oneshot(request_with_ip("203.0.113.1"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("X-RateLimit-Limit").unwrap(), "10");
        assert_eq!(
            response.headers().get("X-RateLimit-Remaining").unwrap(),
            "9"
        );
        assert!(response.headers().contains_key("X-RateLimit-Reset"));
    }

    #[tokio::test]
    async fn over_limit_returns_429() {
        let config = test_config(1);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::new());

        let service = RateLimitService::with_store(
            lambda_http::tower::service_fn(ok_handler),
            Arc::clone(&store),
            Arc::clone(&config),
        );
        let first = service
            .oneshot(request_with_ip("203.0.113.2"))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let service =
            RateLimitService::with_store(lambda_http::tower::service_fn(ok_handler), store, config);
        let second = service
            .oneshot(request_with_ip("203.0.113.2"))
            .await
            .unwrap();

        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(second.headers().contains_key("Retry-After"));
        assert_eq!(second.headers().get("X-RateLimit-Remaining").unwrap(), "0");
    }

    #[tokio::test]
    async fn different_ips_have_separate_counters() {
        let config = test_config(1);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::new());

        let service = RateLimitService::with_store(
            lambda_http::tower::service_fn(ok_handler),
            Arc::clone(&store),
            Arc::clone(&config),
        );
        let first = service
            .oneshot(request_with_ip("203.0.113.10"))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let service =
            RateLimitService::with_store(lambda_http::tower::service_fn(ok_handler), store, config);
        let second = service
            .oneshot(request_with_ip("203.0.113.20"))
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn dynamodb_error_returns_503() {
        let config = test_config(10);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::failing());
        let service =
            RateLimitService::with_store(lambda_http::tower::service_fn(ok_handler), store, config);

        let response = service
            .oneshot(request_with_ip("203.0.113.30"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        // Default 503 deliberately omits Retry-After; users can add one via on_unavailable.
        assert!(!response.headers().contains_key("Retry-After"));
    }

    #[tokio::test]
    async fn custom_over_limit_response_tweaks_pre_built_response() {
        let config = test_config(1);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::new());

        let mut service = RateLimitService::with_store(
            lambda_http::tower::service_fn(ok_handler),
            Arc::clone(&store),
            Arc::clone(&config),
        );
        // Override receives the pre-built default 429 and only tweaks one header.
        service.over_limit = Arc::new(
            |_req: &Request<Body>, mut response: Response<Body>, _ctx: &OverLimitCtx| {
                response.headers_mut().insert(
                    "X-Custom-Header",
                    HeaderValue::from_static("from-customiser"),
                );
                response
            },
        );

        // First request consumes the quota.
        let _ = ServiceExt::oneshot(&mut service, request_with_ip("203.0.113.40"))
            .await
            .unwrap();
        // Second request trips the limit and goes through the customiser.
        let response = service
            .oneshot(request_with_ip("203.0.113.40"))
            .await
            .unwrap();

        // Status, body, and standard headers came from the pre-built response.
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().contains_key("Retry-After"));
        assert_eq!(
            response.headers().get("X-RateLimit-Remaining").unwrap(),
            "0"
        );
        // The customiser only added one header.
        assert_eq!(
            response.headers().get("X-Custom-Header").unwrap(),
            "from-customiser"
        );
    }

    #[tokio::test]
    async fn custom_unavailable_response_replaces_pre_built_response() {
        let config = test_config(10);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::failing());
        let mut service =
            RateLimitService::with_store(lambda_http::tower::service_fn(ok_handler), store, config);
        // Replace the pre-built response entirely with a 502.
        service.unavailable = Arc::new(|_req: &Request<Body>, _response: Response<Body>| {
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("upstream down"))
                .unwrap()
        });

        let response = service
            .oneshot(request_with_ip("203.0.113.41"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn over_limit_callback_sees_request() {
        let config = test_config(1);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::new());

        let mut service = RateLimitService::with_store(
            lambda_http::tower::service_fn(ok_handler),
            Arc::clone(&store),
            Arc::clone(&config),
        );
        service.over_limit = Arc::new(
            |request: &Request<Body>, mut response: Response<Body>, _ctx: &OverLimitCtx| {
                // Echo the original request URI back on a custom header.
                let uri = request.uri().to_string();
                response
                    .headers_mut()
                    .insert("X-Echo-Uri", HeaderValue::from_str(&uri).unwrap());
                response
            },
        );

        let _ = ServiceExt::oneshot(&mut service, request_with_ip("203.0.113.50"))
            .await
            .unwrap();
        let response = service
            .oneshot(request_with_ip("203.0.113.50"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get("X-Echo-Uri").unwrap(),
            "http://example.com/test"
        );
    }

    #[tokio::test]
    async fn missing_ip_falls_through_to_inner() {
        let config = test_config(1);
        let store: Arc<dyn RateLimitStore> = Arc::new(MockRateLimitStore::new());
        let service =
            RateLimitService::with_store(lambda_http::tower::service_fn(ok_handler), store, config);

        let request = Request::builder()
            .method("GET")
            .uri("http://example.com/test")
            .body(Body::Empty)
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        // No RateLimit-* headers, because we skipped the counter entirely.
        assert!(!response.headers().contains_key("X-RateLimit-Limit"));
    }
}
