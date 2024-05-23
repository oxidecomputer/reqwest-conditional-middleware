//! The only export of this crate is a struct [`ConditionalMiddleware`] for creating conditional middlewares.
//! This struct implements the [`Middleware`][reqwest_middleware::Middleware] trait
//! and forwards requests on to the middleware that it wraps.
//!
//! The conditional wrapper holds a closure that will be run for each request. If the
//! closure returns true, then the inner middleware will run. Otherwise it will be
//! skipped and the current request will be passed along to the next middleware.
//!
//! # Example
//!
//! Short-circuits a middleware stack and returns `OK` whenever the request method
//! is `GET`
//!
//! ```
//! use reqwest::{Request, Response};
//! use reqwest_conditional_middleware::ConditionalMiddleware;
//! use reqwest_middleware::{Middleware, Next, Result};
//! use http::Extensions;
//!
//! struct AlwaysOk;
//!
//! #[async_trait::async_trait]
//! impl Middleware for AlwaysOk {
//!     async fn handle(
//!         &self,
//!         _req: Request,
//!         _extensions: &mut Extensions,
//!         _next: Next<'_>,
//!     ) -> Result<Response> {
//!         let builder = http::Response::builder().status(http::StatusCode::OK);
//!         Ok(builder.body("").unwrap().into())
//!     }
//! }
//!
//! let conditional = ConditionalMiddleware::new(
//!     AlwaysOk,
//!     |req: &Request| req.method() == http::Method::GET
//! );
//!
//! ```

use async_trait::async_trait;
use http::Extensions;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next, Result};

/// A struct for holding a [`Middleware`][reqwest_middleware::Middleware] T that will be
/// run when C evaluates to true
pub struct ConditionalMiddleware<T, C> {
    inner: T,
    condition: C,
}

impl<T, C> ConditionalMiddleware<T, C>
where
    T: Middleware,
    C: Fn(&Request) -> bool + Send + Sync + 'static,
{
    /// Creates a new wrapped middleware. The function C will be run for each request to
    /// determine if the wrapped middleware should be run.
    pub fn new(inner: T, condition: C) -> Self {
        Self { inner, condition }
    }
}

#[async_trait]
impl<T, C> Middleware for ConditionalMiddleware<T, C>
where
    T: Middleware,
    C: Fn(&Request) -> bool + Send + Sync + 'static,
{
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        let should_handle = (self.condition)(&req);

        if should_handle {
            self.inner.handle(req, extensions, next).await
        } else {
            next.run(req, extensions).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use reqwest::{Request, Response};
    use std::sync::{Arc, Mutex};

    struct End;

    #[async_trait]
    impl Middleware for End {
        async fn handle(
            &self,
            _req: Request,
            _extensions: &mut Extensions,
            _next: Next<'_>,
        ) -> Result<Response> {
            let builder = http::Response::builder().status(StatusCode::OK);
            let resp = builder.body("end").unwrap();
            Ok(resp.into())
        }
    }

    struct CheckMiddleware {
        check: Arc<Mutex<bool>>,
    }

    impl CheckMiddleware {
        fn new() -> Self {
            Self {
                check: Arc::new(Mutex::new(false)),
            }
        }

        fn flip(&self) {
            let value = *self.check.lock().unwrap();
            *self.check.lock().unwrap() = !value;
        }

        fn checker(&self) -> Arc<Mutex<bool>> {
            self.check.clone()
        }
    }

    #[async_trait]
    impl Middleware for CheckMiddleware {
        async fn handle(
            &self,
            req: Request,
            extensions: &mut Extensions,
            next: Next<'_>,
        ) -> Result<Response> {
            self.flip();
            next.run(req, extensions).await
        }
    }

    #[tokio::test]
    async fn test_runs_inner_middleware() {
        let check = CheckMiddleware::new();
        let test = check.checker();
        let conditional = ConditionalMiddleware::new(check, |_req: &Request| true);
        let request = reqwest::Request::new(http::Method::GET, "http://localhost".parse().unwrap());

        let client =
            reqwest_middleware::ClientBuilder::new(reqwest::Client::builder().build().unwrap())
                .with(conditional)
                .with(End)
                .build();

        let resp = client.execute(request).await.unwrap().text().await.unwrap();

        assert_eq!("end", resp);
        assert!(*test.lock().unwrap())
    }

    #[tokio::test]
    async fn test_does_not_run_inner_middleware() {
        let check = CheckMiddleware::new();
        let test = check.checker();
        let conditional = ConditionalMiddleware::new(check, |_req: &Request| false);
        let request = reqwest::Request::new(http::Method::GET, "http://localhost".parse().unwrap());

        let client =
            reqwest_middleware::ClientBuilder::new(reqwest::Client::builder().build().unwrap())
                .with(conditional)
                .with(End)
                .build();

        let resp = client.execute(request).await.unwrap().text().await.unwrap();

        assert_eq!("end", resp);
        assert!(!*test.lock().unwrap())
    }
}
