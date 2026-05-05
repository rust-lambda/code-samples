//! Extract the client IP from an incoming Lambda HTTP request.
//!
//! This implementation targets **API Gateway** integrations only:
//!
//! * HTTP API (v2) delivers the source IP at
//!   `requestContext.http.sourceIp`.
//! * REST API (v1) delivers it at `requestContext.identity.sourceIp`.
//!
//! Both are populated by AWS from the actual TCP connection, so HTTP
//! clients cannot forge them.
//!
//! Other integrations (Application Load Balancer, AppSync, Lambda
//! Function URLs, CloudFront-fronted setups, …) deliver the client IP
//! through different fields or headers and need a different extractor.

use std::net::IpAddr;
use std::str::FromStr;

use lambda_http::request::RequestContext;
use lambda_http::{Request, RequestExt};

/// Pull the client IP out of an API Gateway request context.
///
/// Returns `None` for any other event shape; callers fail open in that
/// case (see `RateLimitService::call`).
pub fn extract_ip(request: &Request) -> Option<IpAddr> {
    match request.request_context_ref()? {
        RequestContext::ApiGatewayV1(ctx) => ctx
            .identity
            .source_ip
            .as_deref()
            .and_then(|ip| IpAddr::from_str(ip).ok()),
        RequestContext::ApiGatewayV2(ctx) => ctx
            .http
            .source_ip
            .as_deref()
            .and_then(|ip| IpAddr::from_str(ip).ok()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_http::aws_lambda_events::apigw::{
        ApiGatewayProxyRequestContext, ApiGatewayV2httpRequestContext,
        ApiGatewayV2httpRequestContextHttpDescription,
    };
    use lambda_http::Body;

    fn req() -> Request {
        Request::new(Body::Empty)
    }

    fn v2_context_with_source_ip(ip: &str) -> RequestContext {
        let mut http = ApiGatewayV2httpRequestContextHttpDescription::default();
        http.source_ip = Some(ip.to_string());
        let mut ctx = ApiGatewayV2httpRequestContext::default();
        ctx.http = http;
        RequestContext::ApiGatewayV2(ctx)
    }

    fn v1_context_with_source_ip(ip: &str) -> RequestContext {
        let mut ctx = ApiGatewayProxyRequestContext::default();
        ctx.identity.source_ip = Some(ip.to_string());
        RequestContext::ApiGatewayV1(ctx)
    }

    #[test]
    fn http_api_v2_source_ip() {
        let r = req().with_request_context(v2_context_with_source_ip("203.0.113.99"));
        assert_eq!(
            extract_ip(&r),
            Some(IpAddr::from_str("203.0.113.99").unwrap())
        );
    }

    #[test]
    fn rest_api_v1_source_ip() {
        let r = req().with_request_context(v1_context_with_source_ip("203.0.113.42"));
        assert_eq!(
            extract_ip(&r),
            Some(IpAddr::from_str("203.0.113.42").unwrap())
        );
    }

    #[test]
    fn ipv6_source_ip() {
        let r = req().with_request_context(v2_context_with_source_ip("2001:db8::1"));
        assert_eq!(
            extract_ip(&r),
            Some(IpAddr::from_str("2001:db8::1").unwrap())
        );
    }

    #[test]
    fn invalid_source_ip_returns_none() {
        let r = req().with_request_context(v2_context_with_source_ip("not-an-ip"));
        assert_eq!(extract_ip(&r), None);
    }

    #[test]
    fn missing_request_context_returns_none() {
        assert_eq!(extract_ip(&req()), None);
    }

    #[test]
    fn headers_are_ignored() {
        // We deliberately do not look at X-Forwarded-For or
        // CloudFront-Viewer-Address: only the request context counts.
        let mut r = req();
        r.headers_mut()
            .insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        r.headers_mut().insert(
            "cloudfront-viewer-address",
            "5.6.7.8:12345".parse().unwrap(),
        );
        assert_eq!(extract_ip(&r), None);
    }
}
