use anyhow::Result;
use button::{ButtonController, ButtonEvent};
use display::ScenePins;
use esp_idf_svc::{hal::peripherals::Peripherals, sys};
use std::thread;
use std::time::Duration;
mod api;
mod button;
mod display;
mod layout;
mod wifi;

const BUTTON_WAKEUP_MASK: u64 = (1_u64 << 5) | (1_u64 << 6);

fn enter_deep_sleep() -> Result<()> {
    sys::esp!(unsafe {
        sys::esp_sleep_pd_config(
            sys::esp_sleep_pd_domain_t_ESP_PD_DOMAIN_RTC_PERIPH,
            sys::esp_sleep_pd_option_t_ESP_PD_OPTION_ON,
        )
    })?;
    for pin in [sys::gpio_num_t_GPIO_NUM_5, sys::gpio_num_t_GPIO_NUM_6] {
        sys::esp!(unsafe { sys::rtc_gpio_pullup_en(pin) })?;
        sys::esp!(unsafe { sys::rtc_gpio_pulldown_dis(pin) })?;
    }
    sys::esp!(unsafe {
        sys::esp_sleep_enable_ext1_wakeup(
            BUTTON_WAKEUP_MASK,
            sys::esp_sleep_ext1_wakeup_mode_t_ESP_EXT1_WAKEUP_ANY_LOW,
        )
    })?;
    log::info!("Entering deep sleep; GPIO5 or GPIO6 can wake the viewer");
    unsafe { sys::esp_deep_sleep_start() }
}

fn main() -> Result<()> {
    sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let woke_from_deep_sleep = unsafe { sys::esp_sleep_get_wakeup_cause() }
        == sys::esp_sleep_source_t_ESP_SLEEP_WAKEUP_EXT1;
    if woke_from_deep_sleep {
        log::info!("Wake up from deep sleep");
        sys::esp!(unsafe { sys::rtc_gpio_deinit(sys::gpio_num_t_GPIO_NUM_5) })?;
        sys::esp!(unsafe { sys::rtc_gpio_deinit(sys::gpio_num_t_GPIO_NUM_6) })?;
    }

    let peripherals = Peripherals::take()?;

    let (_wifi_subscription, _wifi) = wifi::connect(peripherals.modem)?;
    let mut client = api::ApiClient::new()?;
    let pages = client.get_pages()?;
    let missing = client.get_missing()?;

    let pins = peripherals.pins;
    let display_pins = ScenePins {
        sck: pins.gpio7.degrade_output(),
        mosi: pins.gpio9.degrade_output(),
        cs: pins.gpio1.degrade_output(),
        dc: pins.gpio2.degrade_output(),
        rst: pins.gpio3.degrade_output(),
        busy: pins.gpio4.degrade_input(),
    };
    let mut buttons = ButtonController::init(pins.gpio6, pins.gpio5, woke_from_deep_sleep)?;
    let scene = display::Scene::new(peripherals.spi2, display_pins)?;
    let mut layout = layout::Layout::new(scene, client, pages, missing)?;
    loop {
        if let Some(event) = buttons.poll() {
            match event {
                ButtonEvent::Previous => layout.previous()?,
                ButtonEvent::Next => layout.next()?,
                ButtonEvent::Refresh => {
                    log::info!("Long press -> refresh current page");
                    layout.refresh()?;
                }
                ButtonEvent::Sleep => {
                    log::info!("Left long press -> deep sleep");
                    enter_deep_sleep()?;
                }
                ButtonEvent::Reset => {
                    log::info!("Both buttons long press -> reset server");
                    if let Err(err) = layout.reset() {
                        log::error!("Server reset failed: {:?}", err);
                    }
                }
            }
        }
        layout.check_sleep()?;
        thread::sleep(Duration::from_millis(50));
    }
}
