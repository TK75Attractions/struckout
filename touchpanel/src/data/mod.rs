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

/// Defines a new-type for id.
macro_rules! id_new_type {
    ($new_type:ident($inner_type:ty)) => {
        #[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, PartialEq, Eq, Hash)]
        pub struct $new_type($inner_type);

        impl $new_type {
            pub fn new(value: $inner_type) -> Self {
                Self(value)
            }

            /// Returns inner value of self.
            pub fn into_inner(self) -> $inner_type {
                <$new_type as Into<$inner_type>>::into(self)
            }
        }
    };
}

pub mod game_master;
mod remaining_time;
pub use remaining_time::*;
//pub mod player;
//pub mod projector;

id_new_type!(PlayerId(u32));

id_new_type!(GameId(u32));

id_new_type!(MachineId(u32));
