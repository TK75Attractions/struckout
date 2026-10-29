use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tonic_health::{
    ServingStatus,
    server::{HealthReporter, health_reporter},
};

const HEALTH_PORT: &str = env!("HEALTH_PORT");

/// ヘルスチェック専用の gRPC サーバーを `HEALTH_PORT` で起動する。
///
/// ポートを開けなければ (ほかの ball_tracker が動いているなど)、ログを出して `None` を返す。
/// リプレイなどで 2 つ同時に動かすことがあるので、本体は止めない
pub async fn start(shutdown: CancellationToken) -> Option<(HealthReporter, JoinHandle<()>)> {
    let addr = format!("0.0.0.0:{HEALTH_PORT}");
    match TcpListener::bind(&addr).await {
        Ok(listener) => Some(start_server(listener, shutdown).await),
        Err(error) => {
            tracing::error!(?error, addr, "ヘルスチェックのポートを開けません");
            None
        }
    }
}

/// `listener` でヘルスチェック専用の gRPC サーバーを起動する。
///
/// 入出力の初期化が終わるまでは NOT_SERVING とし、呼び出し元が準備完了後に状態を更新する。
async fn start_server(
    listener: TcpListener,
    shutdown: CancellationToken,
) -> (HealthReporter, JoinHandle<()>) {
    let (reporter, service) = health_reporter();
    reporter
        .set_service_status("", ServingStatus::NotServing)
        .await;

    let server = tokio::spawn(async move {
        if let Err(error) = Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(
                TcpListenerStream::new(listener),
                shutdown.cancelled_owned(),
            )
            .await
        {
            tracing::error!(?error, "ヘルスチェックサーバーでエラーが発生しました");
        }
    });

    (reporter, server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::transport::Endpoint;
    use tonic_health::pb::{
        HealthCheckRequest, health_check_response::ServingStatus as ResponseStatus,
        health_client::HealthClient,
    };

    #[tokio::test]
    async fn reports_not_serving_until_marked_serving() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("空いているローカルポートに bind できる");
        let address = listener
            .local_addr()
            .expect("bind 済み listener のアドレスを取得できる");
        let shutdown = CancellationToken::new();
        let (reporter, server) = start_server(listener, shutdown.clone()).await;
        let channel = Endpoint::from_shared(format!("http://{address}"))
            .expect("テスト用アドレスは有効な URI になる")
            .connect()
            .await
            .expect("起動したヘルスサーバーに接続できる");
        let mut client = HealthClient::new(channel);

        let response = client
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .expect("health check に成功する")
            .into_inner();
        assert_eq!(response.status, ResponseStatus::NotServing as i32);

        reporter
            .set_service_status("", ServingStatus::Serving)
            .await;
        let response = client
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .expect("health check に成功する")
            .into_inner();
        assert_eq!(response.status, ResponseStatus::Serving as i32);

        shutdown.cancel();
        server.await.expect("health server exits after shutdown");
    }
}
