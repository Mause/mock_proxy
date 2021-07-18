use log::warn;
use simple_logger::SimpleLogger;
use std::ops::Index;

#[tokio::test]
async fn test_simple() {
    SimpleLogger::new().init().unwrap();
    let mut proxy = crate::Proxy::new();
    proxy.register(
        crate::Mock::new("GET", "/hello")
            .with_body_from_json(json::object! { hello: "world" })
            .unwrap()
            .with_header("content-type", "application/json")
            .create(),
    );
    proxy.start();
    warn!("Proxy started");
    let certificate = reqwest::Certificate::from_pem(&proxy.get_certificate()).unwrap();
    let client = reqwest::ClientBuilder::new()
        .add_root_certificate(certificate)
        .proxy(reqwest::Proxy::all(&proxy.url()).unwrap())
        .build()
        .unwrap();
    warn!("Client created");
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
