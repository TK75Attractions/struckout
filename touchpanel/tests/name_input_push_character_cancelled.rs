use std::time::Duration;

mod common;

#[test]
fn cancelled() {
    common::name_input::push_character_validation_test(Duration::from_millis(5000), "");
}
