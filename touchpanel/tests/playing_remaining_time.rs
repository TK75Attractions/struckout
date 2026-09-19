use std::time::Duration;

use struckout_proto::types::GameId;
use touchpanel::{
    data::DisplayableRemainingTime, presentation::playing::test_utils::PlayingScreenTest,
};

#[test]
fn remaining_time_updated_when_rem_tx_sends_new_val() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    let test = PlayingScreenTest::new();

    let join = test.vm.listen_session(GameId::new(1));

    slint::spawn_local({
        let rem_tx = test.rem_tx.clone();
        let complete_tx = test.complete_tx.clone();
        let worker = test.worker.clone();
        async move {
            let time = DisplayableRemainingTime { mins: 3, secs: 14 };
            let rem_rx = rem_tx.subscribe();
            rem_tx.send(time).unwrap();

            slint::Timer::single_shot(Duration::from_millis(100), move || {
                assert_eq!(*test.remaining_time.borrow(), time.to_string().as_str());
            });

            // complete and drop session
            complete_tx.send(()).unwrap();
            worker.spawn_cx(async move |cx| {
                cx.drop_session();
            });
        }
    })
    .unwrap();

    slint::spawn_local(async move {
        join.await;
        slint::quit_event_loop().unwrap();
        test.worker.shutdown();
    })
    .unwrap();

    slint::run_event_loop().unwrap();
}
