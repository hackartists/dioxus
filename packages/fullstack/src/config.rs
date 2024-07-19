//! Launch helper macros for fullstack apps
#![allow(unused)]
use crate::prelude::*;
use dioxus_lib::prelude::*;
use std::sync::Arc;

/// Settings for a fullstack app.
pub struct Config {
    #[cfg(feature = "server")]
    pub(crate) server_fn_route: &'static str,

    #[cfg(feature = "server")]
    pub(crate) server_cfg: ServeConfigBuilder,

    #[cfg(feature = "server")]
    pub(crate) addr: std::net::SocketAddr,

    #[cfg(feature = "web")]
    pub(crate) web_cfg: dioxus_web::Config,

    #[cfg(feature = "desktop")]
    pub(crate) desktop_cfg: dioxus_desktop::Config,

    #[cfg(feature = "mobile")]
    pub(crate) mobile_cfg: dioxus_mobile::Config,
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Self {
            #[cfg(feature = "server")]
            server_fn_route: "",
            #[cfg(feature = "server")]
            addr: std::net::SocketAddr::from(([0, 0, 0, 0], 8080)),
            #[cfg(feature = "server")]
            server_cfg: ServeConfigBuilder::new(),
            #[cfg(feature = "web")]
            web_cfg: dioxus_web::Config::default(),
            #[cfg(feature = "desktop")]
            desktop_cfg: dioxus_desktop::Config::default(),
            #[cfg(feature = "mobile")]
            mobile_cfg: dioxus_mobile::Config::default(),
        }
    }
}

impl Config {
    /// Create a new config for a fullstack app.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the address to serve the app on.
    #[cfg(feature = "server")]
    #[cfg_attr(docsrs, doc(cfg(feature = "server")))]
    pub fn addr(self, addr: impl Into<std::net::SocketAddr>) -> Self {
        let addr = addr.into();
        Self { addr, ..self }
    }

    /// Set the route to the server functions.
    #[cfg(feature = "server")]
    #[cfg_attr(docsrs, doc(cfg(feature = "server")))]
    pub fn server_fn_route(self, server_fn_route: &'static str) -> Self {
        Self {
            server_fn_route,
            ..self
        }
    }

    /// Set the incremental renderer config.
    #[cfg(feature = "server")]
    #[cfg_attr(docsrs, doc(cfg(feature = "server")))]
    pub fn incremental(self, cfg: IncrementalRendererConfig) -> Self {
        Self {
            server_cfg: self.server_cfg.incremental(cfg),
            ..self
        }
    }

    /// Set the server config.
    #[cfg(feature = "server")]
    #[cfg_attr(docsrs, doc(cfg(feature = "server")))]
    pub fn server_cfg(self, server_cfg: ServeConfigBuilder) -> Self {
        Self { server_cfg, ..self }
    }

    /// Set the web config.
    #[cfg(feature = "web")]
    #[cfg_attr(docsrs, doc(cfg(feature = "web")))]
    pub fn web_cfg(self, web_cfg: dioxus_web::Config) -> Self {
        Self { web_cfg, ..self }
    }

    /// Set the desktop config.
    #[cfg(feature = "desktop")]
    #[cfg_attr(docsrs, doc(cfg(feature = "desktop")))]
    pub fn desktop_cfg(self, desktop_cfg: dioxus_desktop::Config) -> Self {
        Self {
            desktop_cfg,
            ..self
        }
    }

    /// Set the mobile config.
    #[cfg(feature = "mobile")]
    #[cfg_attr(docsrs, doc(cfg(feature = "mobile")))]
    pub fn mobile_cfg(self, mobile_cfg: dioxus_mobile::Config) -> Self {
        Self { mobile_cfg, ..self }
    }

    #[cfg(feature = "server")]
    #[cfg_attr(docsrs, doc(cfg(feature = "server")))]
    /// Launch a server application
    pub async fn launch_server(
        self,
        build_virtual_dom: impl Fn() -> VirtualDom + Send + Sync + 'static,
    ) {
        let cfg = self.server_cfg.build();
        let server_fn_route = self.server_fn_route;

        #[cfg(feature = "axum")]
        {
            use crate::axum_adapter::{render_handler, DioxusRouterExt};
            use axum::routing::get;
            use tower::ServiceBuilder;

            let ssr_state = SSRState::new(&cfg);
            let router = axum::Router::new().register_server_fns();
            #[cfg(not(any(feature = "desktop", feature = "mobile")))]
            let router = router
                .serve_static_assets(cfg.assets_path.clone())
                .await
                .connect_hot_reload()
                .fallback(get(render_handler).with_state((
                    cfg,
                    Arc::new(build_virtual_dom),
                    ssr_state,
                )));
            #[cfg(feature = "lambda")]
            {
                println!("Running in AWS Lambda");
                // lambda_http::run(router).await.unwrap();
                lambda_runtime::run(LambdaAdapter::from(router)).await.unwrap();
            }
            #[cfg(not(feature = "lambda"))]
            {
                let addr = self.addr;
                println!("Listening on {}", addr);
                let router = router.into_make_service();
                let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
                axum::serve(listener, router).await.unwrap();
            }
        }
    }
}


#[cfg(feature = "lambda")]
#[allow(missing_docs)]
pub struct LambdaAdapter<'a, R, S> {
    service: S,
    _phantom_data: std::marker::PhantomData<&'a R>,

}


#[cfg(feature = "lambda")]
impl<'a, R, S, E> From<S> for LambdaAdapter<'a, R, S>
where
    S: tower::Service<lambda_http::Request, Response = R, Error = E>,
    S::Future: Send + 'a,
    R: lambda_http::IntoResponse,
{
    fn from(service: S) -> Self {
        LambdaAdapter {
            service,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "lambda")]
impl<'a, R, S, E> tower::Service<lambda_http::LambdaEvent<lambda_http::request::LambdaRequest>> for LambdaAdapter<'a, R, S>
where
    S: tower::Service<lambda_http::Request, Response = R, Error = E>,
    S::Future: Send + 'a,
    R: lambda_http::IntoResponse,

{
    type Response = lambda_http::aws_lambda_events::apigw::ApiGatewayProxyResponse;

    type Error = E;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'a>>;

    fn poll_ready(&mut self, cx: &mut core::task::Context<'_>) -> core::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: lambda_http::LambdaEvent<lambda_http::request::LambdaRequest>) -> Self::Future {
        let request_origin = req.payload.request_origin();
        std::env::set_var("AWS_LAMBDA_HTTP_IGNORE_STAGE_IN_PATH","true");
        let event: lambda_http::Request = req.payload.into();

        let call = self.service.call(lambda_http::RequestExt::with_lambda_context(event, req.context));

        Box::pin(async move {
            let res = call.await?;
            let res = res.into_response().await;
            let status_code = res.status().as_u16() as i64;
            let headers = res.headers().clone();
            let body = Some(res.clone().into_body());

            let res = lambda_http::aws_lambda_events::apigw::ApiGatewayProxyResponse {
                is_base64_encoded: false,
                status_code,
                headers,
                body,
                multi_value_headers: Default::default(),
            };

            Ok(res)
        })
    }
}
