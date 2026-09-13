use anyhow::Result;
use button::{ButtonController, ButtonEvent};
use display::ScenePins;
use esp_idf_svc::hal::gpio::{InputPin, OutputPin};
use esp_idf_svc::hal::prelude::Peripherals;
use std::thread;
use std::time::Duration;
mod api;
mod button;
mod display;
mod layout;
mod wifi;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;

    let _wifi = wifi::connect(peripherals.modem)?;
    let mut client = api::ApiClient::new()?;
    let pages = client.get_pages()?;
    let missing = client.get_missing()?;

    let pins = peripherals.pins;
    let display_pins = ScenePins {
        sck: pins.gpio7.downgrade_output(),
        mosi: pins.gpio9.downgrade_output(),
        cs: pins.gpio1.downgrade_output(),
        dc: pins.gpio2.downgrade_output(),
        rst: pins.gpio3.downgrade_output(),
        busy: pins.gpio4.downgrade_input(),
    };
    let mut buttons = ButtonController::init(pins.gpio6, pins.gpio5)?;
    let scene = display::Scene::new(peripherals.spi2, display_pins)?;
    let mut layout = layout::Layout::new(scene, client, pages, missing)?;
    loop {
        if let Some(event) = buttons.poll() {
            match event {
                ButtonEvent::Previous => layout.previous()?,
                ButtonEvent::Next => layout.next()?,
                ButtonEvent::Reset => layout.reset()?,
            }
        }
        layout.refresh_if_due()?;
        layout.check_sleep()?;
        thread::sleep(Duration::from_millis(50));
    }
}
