# reqwest-conditional-middleware

[![CI](https://github.com/oxidecomputer/reqwest-conditional-middleware/workflows/CI/badge.svg)](https://github.com/oxidecomputer/reqwest-conditional-middleware/actions?query=workflow%3ACI) [![docs.rs](https://docs.rs/reqwest-conditional-middleware/badge.svg)](https://docs.rs/reqwest-conditional-middleware)

A middleware wrapper that enables (or disables) a wrapped [Reqwest middleware](https://github.com/TrueLayer/reqwest-middleware) on a per-request basis

### Example

Usage of this crate depends on a few crates:

```toml
async-trait = "0.1.80"
http = "1"
reqwest = version = "0.12"
reqwest-conditional-middleware = "0.3.0"
reqwest-middleware = "0.4"
```

This is an example of a conditional middleware that short-circuits a middleware stack and
returns `OK` whenever the request method is `GET`

```rust
use reqwest::{Request, Response};
use reqwest_conditional_middleware::ConditionalMiddleware;
use reqwest_middleware::{Middleware, Next, Result};
use task_local_extensions::Extensions;

struct AlwaysOk;

#[async_trait::async_trait]
impl Middleware for AlwaysOk {
    async fn handle(
        &self,
        _req: Request,
        _extensions: &mut Extensions,
        _next: Next<'_>,
    ) -> Result<Response> {
        let builder = http::Response::builder().status(http::StatusCode::OK);
        Ok(builder.body("").unwrap().into())
    }
}

let conditional = ConditionalMiddleware::new(
    AlwaysOk,
    |req: &Request| req.method() == http::Method::GET
);

```