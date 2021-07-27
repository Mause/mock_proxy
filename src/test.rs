use crate::{Mock, Proxy};
use log::warn;
use simple_logger::SimpleLogger;
use std::ops::Index;

fn build_client(proxy: &Proxy) -> reqwest::Client {
    let certificate = reqwest::Certificate::from_pem(&proxy.get_certificate()).unwrap();
    let client = reqwest::ClientBuilder::new()
        .add_root_certificate(certificate)
        .proxy(reqwest::Proxy::all(&proxy.url()).unwrap())
        .build()
        .unwrap();
    warn!("Client created");
    return client;
}

#[tokio::test]
async fn test_simple() {
    SimpleLogger::new().init().unwrap();
    let mut proxy = Proxy::new();
    proxy.register(
        Mock::new("GET", "/hello")
            .with_body_from_json(json::object! { hello: "world" })
            .unwrap()
            .with_header("content-type", "application/json")
            .with_status(201)
            .create(),
    );
    proxy.start();
    warn!("Proxy started");
    let client = build_client(&proxy);
    let response = client
        .get("https://discord.com/hello")
        .send()
        .await
        .unwrap();
    warn!("Request recieved");

    assert_eq!(response.status(), 201);
    assert_eq!(response.headers()["content-type"], "application/json");

    let text = response.text().await.unwrap();

    assert_eq!(json::parse(&text).unwrap().index("hello"), "world");
}

#[tokio::test]
async fn test_domain_matching() {
    let mut proxy = Proxy::default();

    proxy.register(Mock::new("GET", "https://hello.com/path").create());

    proxy.start();

    let client = build_client(&proxy);

    client.get("https://hello.com/path").send().await.unwrap();
}

#[tokio::test]
async fn test_query_params() {
    let mut proxy = Proxy::default();

    let mock = Mock::new("GET", "/path?hello=world").create();
    warn!("mock: {:?}", mock);
    proxy.register(mock);

    proxy.start();

    let client = build_client(&proxy);

    let res = client
        .get("https://hello.com/path?hello=world")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn test_errors() {
    let mut proxy = Proxy::default();

    proxy.start();

    let client = build_client(&proxy);

    let response = client.get("https://hello.com/path").send().await.unwrap();
    assert_eq!(response.status(), 500);

    let response = response.text().await.unwrap();

    assert_eq!(response, "No matching response\r\n");
}

#[tokio::test]
async fn test_http() {
    let mut proxy = Proxy::default();
    proxy.start();

    let client = build_client(&proxy);

    let response = client.get("http://localhost/hello").send().await.unwrap();
    assert_eq!(response.status(), 200);
}
