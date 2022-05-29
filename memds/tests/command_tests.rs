use memds::{
    client::Client,
    command::{
        connection::PingCommand,
        string::{GetCommand, IncrCommand},
    },
    Server,
};

#[tokio::test]
async fn test_ping_command() {
    let port = 0;
    let server = Server::new(port, "/dev/null".into());

    let (addr, server_handle) = server.service().await.unwrap();

    let mut client = Client::from_addr(addr).await.unwrap();
    let result = client.execute(&PingCommand).await.unwrap();
    assert_eq!(result.0, "PONG");

    server_handle.await;
}

#[tokio::test]
async fn test_get_command() {
    let port = 1234;
    let server = Server::new(port, "/dev/null".into());

    let (addr, server_handle) = server.service().await.unwrap();

    let mut client = Client::from_addr(addr).await.unwrap();
    let result = client.execute(&GetCommand { key: "a" }).await.unwrap();
    assert_eq!(result, None);

    server_handle.await;
}

#[tokio::test]
async fn test_incr_command() {
    let port = 1235;
    let server = Server::new(port, "/dev/null".into());

    let (addr, server_handle) = server.service().await.unwrap();

    let mut client = Client::from_addr(addr).await.unwrap();
    let result = client.execute(&IncrCommand { key: "a" }).await.unwrap();
    assert_eq!(result, 1);

    let result = client.execute(&GetCommand { key: "a" }).await.unwrap();
    assert_eq!(result, Some("1".to_string()));

    let result = client.execute(&IncrCommand { key: "a" }).await.unwrap();
    assert_eq!(result, 2);

    let result = client.execute(&GetCommand { key: "a" }).await.unwrap();
    assert_eq!(result, Some("2".to_string()));

    server_handle.await;
}
