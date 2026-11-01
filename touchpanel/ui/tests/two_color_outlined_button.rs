//! Renders `TwoColorOutlinedButton` with Slint's software renderer to check its colors.
//!
//! This file has a single test because a process can set Slint's platform only once.

use std::rc::Rc;

use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle as _, PhysicalSize, Rgb8Pixel,
    platform::{
        Platform, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use touchpanel_ui::{AppWindow, NavRouteKind};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

struct OffscreenPlatform(Rc<MinimalSoftwareWindow>);

impl Platform for OffscreenPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

/// Like the logo, the button is blue on the left and yellow on the right.
///
/// The emphasized button is filled with the same conic gradient as the outline.
/// The software renderer draws gradient backgrounds but not gradient borders, so the fill is checked.
#[test]
fn emphasized_button_is_blue_on_left_and_yellow_on_right() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(OffscreenPlatform(window.clone()))).unwrap();
    let ui = AppWindow::new().unwrap();
    // NORMAL is emphasized because it is the initial difficulty.
    ui.set_nav_route(NavRouteKind::DifficulitySelect.into());
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    ui.show().unwrap();

    let mut buffer = vec![Rgb8Pixel::default(); (WIDTH * HEIGHT) as usize];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, WIDTH as usize);
    });

    let button = ElementHandle::find_by_accessible_label(&ui, "NORMAL")
        .next()
        .expect("NORMAL button should be shown");
    let position = button.absolute_position();
    let size = button.size();
    // Sample beside the text on the horizontal center line.
    let pixel_at = |ratio: f32| {
        let x = (position.x + size.width * ratio) as u32;
        let y = (position.y + size.height / 2.) as u32;
        buffer[(y * WIDTH + x) as usize]
    };
    let left = pixel_at(0.08);
    let right = pixel_at(0.92);

    // StruckoutPalette.ref_primary_70 (#9da3ff) is blue and ref_secondary_70 (#b1b100) is yellow.
    assert!(left.b > 200 && left.r < 180, "left should be blue: {left:?}");
    assert!(right.b < 50 && right.r > 150, "right should be yellow: {right:?}");
}
