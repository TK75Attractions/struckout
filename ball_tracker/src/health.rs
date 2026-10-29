use std::sync::Arc;

use ball_tracker::camera_status::{CameraStatusServiceImpl, CameraStatusStore};
use struckout_proto::camera_status_service_server::CameraStatusServiceServer;
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

/// ヘルスチェックとカメラ状態の gRPC サーバーを `HEALTH_PORT` で起動する。
///
/// ポートを開けなければ (ほかの ball_tracker が動いているなど)、ログを出して `None` を返す。
/// リプレイなどで 2 つ同時に動かすことがあるので、本体は止めない
pub async fn start(
    shutdown: CancellationToken,
    camera_status: Arc<CameraStatusStore>,
) -> Option<(HealthReporter, JoinHandle<()>)> {
    let addr = format!("0.0.0.0:{HEALTH_PORT}");
    match TcpListener::bind(&addr).await {
        Ok(listener) => Some(start_server(listener, shutdown, camera_status).await),
        Err(error) => {
            tracing::error!(?error, addr, "ヘルスチェックのポートを開けません");
            None
        }
    }
}

/// `listener` でヘルスチェックとカメラ状態の gRPC サーバーを起動する。
///
/// 入出力の初期化が終わるまでは NOT_SERVING とし、呼び出し元が準備完了後に状態を更新する。
async fn start_server(
    listener: TcpListener,
    shutdown: CancellationToken,
    camera_status: Arc<CameraStatusStore>,
) -> (HealthReporter, JoinHandle<()>) {
    let (reporter, service) = health_reporter();
    reporter
        .set_service_status("", ServingStatus::NotServing)
        .await;

    let server = tokio::spawn(async move {
        if let Err(error) = Server::builder()
            .add_service(service)
            .add_service(CameraStatusServiceServer::new(CameraStatusServiceImpl(
                camera_status,
            )))
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
    use ball_tracker::{camera_config::CameraConfig, types::CameraId};
    use struckout_proto::{
        ListCameraStatusRequest, camera_status_service_client::CameraStatusServiceClient,
    };
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
        let (reporter, server) = start_server(
            listener,
            shutdown.clone(),
            Arc::new(CameraStatusStore::new(None)),
        )
        .await;
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

    #[tokio::test]
    async fn lists_camera_status_on_the_health_server() {
        let config = CameraConfig::parse(
            r#"{"cameras":[{"name":"left","device_id":"aaa","pose":[0,0,0,0,0,0]}]}"#,
        )
        .expect("テストの設定は有効");
        let store = Arc::new(CameraStatusStore::new(Some(&config)));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("空きポートに bind できる");
        let address = listener
            .local_addr()
            .expect("bind 済みのアドレスを取得できる");
        let shutdown = CancellationToken::new();
        let (_, server) = start_server(listener, shutdown.clone(), store.clone()).await;
        let channel = Endpoint::from_shared(format!("http://{address}"))
            .expect("有効な URI")
            .connect()
            .await
            .expect("起動したサーバーに接続できる");
        let mut health = HealthClient::new(channel.clone());
        health
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .expect("同じポートで health にも応答する");
        let mut client = CameraStatusServiceClient::new(channel);
        let initial = client
            .list_camera_status(ListCameraStatusRequest {})
            .await
            .expect("一覧を取得できる")
            .into_inner();
        assert_eq!(initial.cameras.len(), 1);
        assert_eq!(initial.cameras[0].name, "left");
        assert!(!initial.cameras[0].connected);
        assert!(initial.cameras[0].last_frame_at.is_none());

        let id = CameraId::new(0);
        let connection = store.connect(id, "aaa");
        store.received(
            id,
            chrono::DateTime::from_timestamp(100, 0).expect("有効な時刻"),
        );
        let received = client
            .list_camera_status(ListCameraStatusRequest {})
            .await
            .expect("一覧を取得できる")
            .into_inner();
        assert!(received.cameras[0].connected);
        assert_eq!(
            received.cameras[0]
                .last_frame_at
                .as_ref()
                .map(|at| at.seconds),
            Some(100)
        );
        drop(connection);
        let disconnected = client
            .list_camera_status(ListCameraStatusRequest {})
            .await
            .expect("一覧を取得できる")
            .into_inner();
        assert!(!disconnected.cameras[0].connected);
        assert!(disconnected.cameras[0].disconnected_at.is_some());
        assert_eq!(
            disconnected.cameras[0].last_frame_at,
            received.cameras[0].last_frame_at
        );

        shutdown.cancel();
        server.await.expect("停止要求でサーバーが終了する");
    }
}
