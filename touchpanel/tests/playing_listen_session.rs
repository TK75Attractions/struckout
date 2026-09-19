use struckout_proto::types::GameId;
use touchpanel::presentation::playing::test_utils::PlayingScreenTest;

#[test]
fn listen_session_does_not_panic_when_session_completes() {
    i_slint_backend_testing::init_integration_test_with_system_time();

    let test = PlayingScreenTest::new();

    let join = test.vm.listen_session(GameId::new(1));

    // complete and drop session.
    test.complete_tx.send(()).unwrap();
    test.worker.spawn_cx(async move |cx| {
        cx.drop_session();
    });

    slint::spawn_local(async move {
        join.await;
        slint::quit_event_loop().unwrap();
        test.worker.shutdown();
    })
    .unwrap();

    slint::run_event_loop().unwrap();
}
