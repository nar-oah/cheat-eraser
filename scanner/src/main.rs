use esp_idf_svc::hal::gpio::{PinDriver, Pull};
use esp_idf_svc::hal::peripherals::Peripherals;
use std::sync::mpsc::{self, TrySendError};
use std::thread;
use std::time::{Duration, Instant};
mod camera;
mod upload;
mod wifi;

const LONG_PRESS_DURATION: Duration = Duration::from_millis(2500);
const RELEASE_STABLE_DURATION: Duration = Duration::from_millis(100);

fn enter_deep_sleep() -> anyhow::Result<()> {
    esp_idf_sys::esp!(unsafe {
        esp_idf_sys::esp_sleep_pd_config(
            esp_idf_sys::esp_sleep_pd_domain_t_ESP_PD_DOMAIN_RTC_PERIPH,
            esp_idf_sys::esp_sleep_pd_option_t_ESP_PD_OPTION_ON,
        )
    })?;
    esp_idf_sys::esp!(unsafe {
        esp_idf_sys::rtc_gpio_pullup_en(esp_idf_sys::gpio_num_t_GPIO_NUM_2)
    })?;
    esp_idf_sys::esp!(unsafe {
        esp_idf_sys::rtc_gpio_pulldown_dis(esp_idf_sys::gpio_num_t_GPIO_NUM_2)
    })?;
    esp_idf_sys::esp!(unsafe {
        esp_idf_sys::esp_sleep_enable_ext1_wakeup(
            1_u64 << 2,
            esp_idf_sys::esp_sleep_ext1_wakeup_mode_t_ESP_EXT1_WAKEUP_ANY_LOW,
        )
    })?;
    log::info!("Entering deep sleep; GPIO2 can wake the scanner");
    unsafe { esp_idf_sys::esp_deep_sleep_start() }
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let woke_from_deep_sleep = unsafe { esp_idf_sys::esp_sleep_get_wakeup_cause() }
        == esp_idf_sys::esp_sleep_source_t_ESP_SLEEP_WAKEUP_EXT1;
    if woke_from_deep_sleep {
        log::info!("Wake up from deep sleep");
    }
    let peripherals = Peripherals::take()?;

    let _wifi = wifi::connect(peripherals.modem)?;
    camera::init()?;

    esp_idf_sys::esp!(unsafe { esp_idf_sys::rtc_gpio_deinit(esp_idf_sys::gpio_num_t_GPIO_NUM_2) })?;
    let button = PinDriver::input(peripherals.pins.gpio2, Pull::Up)?;
    let mut prev_is_pressed = false;
    let mut pressed_at = None;
    let mut is_long_press = false;
    let mut waiting_for_release = woke_from_deep_sleep;
    let mut released_at: Option<Instant> = None;

    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
    thread::Builder::new()
        .stack_size(1024 * 10)
        .spawn(move || {
            log::info!("Upload thread started");
            while let Ok(data) = rx.recv() {
                log::info!("Background thread: received image, uploading...");
                match upload::image(data) {
                    Ok(result) if result.accepted => log::info!(
                        "Paper accepted: page={:?}, variance={:?}",
                        result.page,
                        result.variance
                    ),
                    Ok(result) => log::warn!(
                        "HTTP upload succeeded, but paper was rejected: reason={}, variance={:?}",
                        result.reject_reason.as_deref().unwrap_or("unknown"),
                        result.variance
                    ),
                    Err(e) => log::error!("Upload failed: {:?}", e),
                }
                log::info!("Background thread: ready for next task");
            }
        })?;

    loop {
        let is_pressed = button.is_low();

        if waiting_for_release {
            if is_pressed {
                released_at = None;
            } else if released_at.get_or_insert_with(Instant::now).elapsed()
                >= RELEASE_STABLE_DURATION
            {
                waiting_for_release = false;
                released_at = None;
                log::info!("Wake button released; button input ready");
            }
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        if is_pressed != prev_is_pressed {
            if is_pressed {
                if is_long_press {
                    released_at = None;
                } else {
                    log::info!("👇 按钮被按下了！ (Pressed)");
                    pressed_at = Some(Instant::now());
                }
            } else if is_long_press
                || pressed_at.is_some_and(|started| started.elapsed() >= LONG_PRESS_DURATION)
            {
                is_long_press = true;
                released_at = Some(Instant::now());
                log::info!("Long press released; waiting for stable button state");
            } else if pressed_at.is_some() {
                log::info!("Short press -> capture");
                let free = unsafe { esp_idf_sys::esp_get_free_heap_size() };
                log::info!("Current Free Heap: {} bytes", free);

                if let Some(frame) = camera::CameraFrame::get() {
                    let data = frame.data().to_vec();
                    log::info!("Picture taken! Size: {} bytes. Queueing...", data.len());
                    match tx.try_send(data) {
                        Ok(()) => log::info!("Image queued for upload"),
                        Err(TrySendError::Full(data)) => {
                            drop(data);
                            log::warn!("Upload queue busy; dropped newly captured image");
                        }
                        Err(TrySendError::Disconnected(data)) => {
                            drop(data);
                            log::error!("Upload thread stopped; dropped newly captured image");
                        }
                    }
                } else {
                    log::error!("Camera Capture Failed");
                }
            }
            prev_is_pressed = is_pressed;
        }

        if is_pressed
            && !is_long_press
            && pressed_at.is_some_and(|started| started.elapsed() >= LONG_PRESS_DURATION)
        {
            is_long_press = true;
            log::info!("Long press detected; release button to enter deep sleep");
        }

        if is_long_press
            && !is_pressed
            && released_at.is_some_and(|started| started.elapsed() >= RELEASE_STABLE_DURATION)
        {
            enter_deep_sleep()?;
        }

        thread::sleep(Duration::from_millis(50));
    }
}
