use std::time::Duration;

mod common;

#[test]
fn not_cancelled() {
    common::name_input::push_character_validation_test(
        Duration::from_millis(1000),
        "プレイヤー名は既に使われています",
    );
}
