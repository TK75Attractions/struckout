use std::{cell::Cell, rc::Rc, time::Duration};

use i_slint_backend_testing::mock_elapsed_time;
use slint::ComponentHandle as _;
use touchpanel_ui::{AppWindow, GameFinishedAdopter, NavRouteKind};

/// Shows `GameFinishedScreen` and counts requests to return home.
fn show_game_finished_screen() -> (AppWindow, Rc<Cell<usize>>) {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let requests = Rc::new(Cell::new(0));
    ui.global::<GameFinishedAdopter>().on_move_to_start_screen({
        let requests = Rc::clone(&requests);
        move || requests.set(requests.get() + 1)
    });
    ui.set_nav_route(NavRouteKind::GameFinished.into());
    ui.show().unwrap();
    (ui, requests)
}

/// Applies property changes to elements such as `Timer` and `if`, as the event loop does right after the change.
///
/// [`mock_elapsed_time()`] fires timers before it runs the change handlers,
/// so a change made just before it would otherwise apply only after the elapsed time.
fn apply_changes() {
    mock_elapsed_time(Duration::ZERO);
}

#[test]
fn returns_home_after_20_seconds() {
    let (_ui, requests) = show_game_finished_screen();

    mock_elapsed_time(Duration::from_secs(19));
    assert_eq!(requests.get(), 0);

    mock_elapsed_time(Duration::from_secs(1));
    assert_eq!(requests.get(), 1);
}

#[test]
fn does_not_request_again_while_busy() {
    let (ui, requests) = show_game_finished_screen();
    mock_elapsed_time(Duration::from_secs(20));
    assert_eq!(requests.get(), 1);

    ui.global::<GameFinishedAdopter>().set_busy(true);
    apply_changes();
    mock_elapsed_time(Duration::from_secs(40));
    assert_eq!(requests.get(), 1);
}

#[test]
fn retries_20_seconds_after_failure() {
    let (ui, requests) = show_game_finished_screen();
    let adopter = ui.global::<GameFinishedAdopter>();
    adopter.set_busy(true);
    apply_changes();
    mock_elapsed_time(Duration::from_secs(30));
    assert_eq!(requests.get(), 0);

    adopter.set_busy(false);
    apply_changes();
    mock_elapsed_time(Duration::from_secs(19));
    assert_eq!(requests.get(), 0);
    mock_elapsed_time(Duration::from_secs(1));
    assert_eq!(requests.get(), 1);
}

#[test]
fn leaving_screen_stops_timer() {
    let (ui, requests) = show_game_finished_screen();
    mock_elapsed_time(Duration::from_secs(10));

    ui.set_nav_route(NavRouteKind::Start.into());
    apply_changes();
    mock_elapsed_time(Duration::from_secs(20));
    assert_eq!(requests.get(), 0);
}
