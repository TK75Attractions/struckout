use std::{future::Future, time::Duration};

use tonic_health::{ServingStatus, server::HealthReporter};
use tracing::{info, warn};

const CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Updates the gRPC health status using the result of a database check.
/// The check runs immediately and then every five seconds.
pub async fn monitor_database<F, Fut>(
    reporter: HealthReporter,
    service_names: Vec<&'static str>,
    mut check: F,
) -> !
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    reporter
        .set_service_status("", ServingStatus::NotServing)
        .await;
    for name in &service_names {
        reporter
            .set_service_status(name, ServingStatus::NotServing)
            .await;
    }

    let mut is_serving = false;
    loop {
        let next_is_serving = check().await;
        if next_is_serving != is_serving {
            if next_is_serving {
                info!("database is available; gRPC health is SERVING");
            } else {
                warn!("database is unavailable; gRPC health is NOT_SERVING");
            }
            is_serving = next_is_serving;
        }

        let status = if is_serving {
            ServingStatus::Serving
        } else {
            ServingStatus::NotServing
        };
        reporter.set_service_status("", status).await;
        for name in &service_names {
            reporter.set_service_status(name, status).await;
        }

        tokio::time::sleep(CHECK_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use rstest::rstest;
    use tonic::Request;
    use tonic_health::{
        pb::{
            HealthCheckRequest, health_check_response::ServingStatus as Status,
            health_server::Health as _,
        },
        server::{HealthService, health_reporter},
    };

    use super::*;

    const SERVICE_NAME: &str = "test.GameMasterService";

    /// Fetches a service's status to verify both server-wide and named health checks.
    async fn status(health: &HealthService, service_name: &str) -> Status {
        health
            .check(Request::new(HealthCheckRequest {
                service: service_name.into(),
            }))
            .await
            .expect("health check should succeed")
            .into_inner()
            .status()
    }

    /// Moves the paused clock a second at a time, letting the monitor run after each step.
    async fn wait(duration: Duration) {
        for _ in 0..duration.as_secs() {
            tokio::time::advance(Duration::from_secs(1)).await;
            tokio::task::yield_now().await;
        }
    }

    #[rstest]
    #[case::whole_server("")]
    #[case::named_service(SERVICE_NAME)]
    #[tokio::test(start_paused = true)]
    async fn follows_the_database_check(#[case] service_name: &str) {
        let (reporter, _) = health_reporter();
        let health = HealthService::from_health_reporter(reporter.clone());
        let mut results = VecDeque::from([true, false, true]);
        // Each check takes a second, so the status before its result can be seen.
        let check = move || {
            let result = results.pop_front().unwrap();
            async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                result
            }
        };
        let task = tokio::spawn(monitor_database(reporter, vec![SERVICE_NAME], check));

        tokio::task::yield_now().await;
        assert_eq!(status(&health, service_name).await, Status::NotServing);

        wait(Duration::from_secs(1)).await;
        assert_eq!(status(&health, service_name).await, Status::Serving);

        wait(CHECK_INTERVAL + Duration::from_secs(1)).await;
        assert_eq!(status(&health, service_name).await, Status::NotServing);

        wait(CHECK_INTERVAL + Duration::from_secs(1)).await;
        assert_eq!(status(&health, service_name).await, Status::Serving);

        task.abort();
    }
}
