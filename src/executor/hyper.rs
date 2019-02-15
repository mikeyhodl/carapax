use crate::executor::{Executor, ExecutorError};
use crate::methods::{Request, RequestBody, RequestMethod};
use futures::{future, Future, Stream};
use hyper::{
    client::{connect::Connect, Client, HttpConnector},
    Body, Request as HttpRequest,
};
use hyper_proxy::{
    Intercept as HttpProxyIntercept, Proxy as HttpProxy, ProxyConnector as HttpProxyConnector,
};
use hyper_socks2::{Auth as SocksAuth, Connector as SocksProxyConnector, Proxy as SocksProxy};
use hyper_tls::HttpsConnector;
use log::{debug, log_enabled, Level::Debug};
use native_tls::TlsConnector;
use std::net::SocketAddr;
use std::rc::Rc;
use typed_headers::Credentials as HttpProxyCredentials;
use url::percent_encoding::percent_decode;
use url::Url;

const DEFAULT_HTTPS_DNS_WORKER_THREADS: usize = 1;

struct HyperExecutor<C> {
    client: Rc<Client<C>>,
}

impl<C> HyperExecutor<C> {
    fn new(client: Client<C>) -> Self {
        HyperExecutor {
            client: Rc::new(client),
        }
    }
}

impl<C: Connect + 'static> Executor for HyperExecutor<C> {
    fn execute(&self, req: Request) -> Box<Future<Item = Vec<u8>, Error = ExecutorError>> {
        let mut builder = match req.method {
            RequestMethod::Get => HttpRequest::get(req.url),
            RequestMethod::Post => HttpRequest::post(req.url),
        };
        let client = self.client.clone();
        Box::new(
            future::result(match req.body {
                RequestBody::Json(data) => {
                    if log_enabled!(Debug) {
                        debug!("Post JSON data: {}", String::from_utf8_lossy(&data));
                    }
                    builder.header("Content-Type", "application/json");
                    builder.body(data.into())
                }
                RequestBody::Empty => builder.body(Body::empty()),
            })
            .map_err(ExecutorError::from)
            .and_then(move |http_req| client.request(http_req).map_err(ExecutorError::from))
            .and_then(|rep| {
                Stream::fold(
                    rep.into_body().from_err(),
                    Vec::new(),
                    |mut out, chunk| -> Result<Vec<u8>, ExecutorError> {
                        out.extend_from_slice(&chunk);
                        Ok(out)
                    },
                )
                .and_then(|body| {
                    if log_enabled!(Debug) {
                        debug!("Got response: {}", String::from_utf8_lossy(&body));
                    }
                    Ok(body)
                })
            }),
        )
    }
}

fn https_connector() -> Result<HttpsConnector<HttpConnector>, ExecutorError> {
    Ok(HttpsConnector::new(DEFAULT_HTTPS_DNS_WORKER_THREADS)?)
}

pub(crate) fn default_executor() -> Result<Box<Executor>, ExecutorError> {
    let connector = https_connector()?;
    let client = Client::builder().build(connector);
    Ok(Box::new(HyperExecutor::new(client)))
}

fn socks_proxy_executor(proxy: SocksProxy<SocketAddr>) -> Result<Box<Executor>, ExecutorError> {
    let connector = HttpsConnector::from((SocksProxyConnector::new(proxy), TlsConnector::new()?));
    let client = Client::builder().build(connector);
    Ok(Box::new(HyperExecutor::new(client)))
}

fn http_proxy_executor(proxy: HttpProxy) -> Result<Box<Executor>, ExecutorError> {
    let connector = https_connector()?;
    let proxy_connector = HttpProxyConnector::from_proxy(connector, proxy)?;
    let client = Client::builder().build(proxy_connector);
    Ok(Box::new(HyperExecutor::new(client)))
}

pub(crate) fn proxy_executor(dsn: &str) -> Result<Box<Executor>, ExecutorError> {
    macro_rules! unexpected_proxy {
        () => {
            return Err(ExecutorError::UnexpectedProxy(dsn.to_string()));
        };
    }
    let parsed_dsn = Url::parse(dsn)?;
    let host: SocketAddr = match (parsed_dsn.host_str(), parsed_dsn.port()) {
        (Some(host), Some(port)) => format!("{}:{}", host, port).parse()?,
        _ => unexpected_proxy!(),
    };
    match parsed_dsn.scheme() {
        "http" | "https" => {
            let mut proxy = HttpProxy::new(HttpProxyIntercept::All, dsn.parse()?);
            if let Some(password) = parsed_dsn.password() {
                proxy.set_authorization(HttpProxyCredentials::basic(
                    parsed_dsn.username(),
                    password,
                )?);
            }
            http_proxy_executor(proxy)
        }
        "socks4" => socks_proxy_executor(SocksProxy::Socks4 {
            addrs: host,
            user_id: parsed_dsn.username().to_string(),
        }),
        "socks5" => socks_proxy_executor(SocksProxy::Socks5 {
            addrs: host,
            auth: parsed_dsn.password().map(|password| SocksAuth {
                user: parsed_dsn.username().to_string(),
                pass: percent_decode(password.as_bytes())
                    .decode_utf8_lossy()
                    .to_string(),
            }),
        }),
        _ => unexpected_proxy!(),
    }
}
