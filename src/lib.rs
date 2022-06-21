use async_trait::async_trait;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next, Result};
use task_local_extensions::Extensions;

pub struct ConditionalMiddleware<T, C> {
    inner: T,
    condition: C,
}

impl<T, C> ConditionalMiddleware<T, C>
where
    T: Middleware,
    C: Fn(&Request) -> bool + Send + Sync + 'static,
{
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
