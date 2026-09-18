//! Similar to Data Layer in Android.
//! This module deals with data sources (e.g. database).

/// Defines method which wraps inner async method.
macro_rules! async_wrapper {
    (
        $(#[$meta:meta])*
        $name:ident()
    ) => {
        pastey::paste! {
            async_wrapper!(
                $(#[$meta])*
                $name() -> [<$name:camel Error>]
            );
        }
    };
    (
        $(#[$meta:meta])*
        $name:ident() -> $error:ty
    ) => {
        pastey::paste! {
            $(#[$meta])*
            pub fn [<$name:snake>]<F>(&self, cb: F)
            where
                F: FnOnce(Result<(), $error>) + 'static,
            {
                use tokio::sync::oneshot;
                let (res_tx, res_rx) = oneshot::channel();
                self.msg_tx
                    .blocking_send((Command::[<$name:camel>], res_tx))
                    .unwrap();
                slint::spawn_local(async move {
                    let Response::[<$name:camel>](res) = res_rx.await.unwrap() else {
                        panic!(
                            "Command::{} should return Response::{}",
                            stringify!([<$name:camel>]),
                            stringify!([<$name:camel>]),
                        );
                    };
                    cb(res);
                }).unwrap();
            }
        }
    };
}

mod game_master;
pub use game_master::{ConnectError, RequestError, Session};
mod remaining_time;
pub use remaining_time::*;
use struckout_proto::game_master_service_client::GameMasterServiceClient;

pub type GameMasterClient =
    game_master::GameMasterClient<GameMasterServiceClient<tonic::transport::Channel>>;
//pub mod player;
//pub mod projector;
