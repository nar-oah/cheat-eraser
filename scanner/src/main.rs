use esp_idf_svc::hal::gpio::{PinDriver, Pull};
use esp_idf_svc::hal::peripherals::Peripherals;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
mod camera;
mod upload;
mod wifi;

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

    esp_idf_sys::esp!(unsafe {
        esp_idf_sys::rtc_gpio_deinit(esp_idf_sys::gpio_num_t_GPIO_NUM_2)
    })?;
    let button = PinDriver::input(peripherals.pins.gpio2, Pull::Up)?;
    let mut prev_is_pressed = false;
    let mut pressed_at = None;
    let mut is_long_press = false;
    let mut waiting_for_release = woke_from_deep_sleep;

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::Builder::new()
        .stack_size(1024 * 10)
        .spawn(move || {
            log::info!("Upload thread started");
            while let Ok(data) = rx.recv() {
                log::info!("Background thread: received image, uploading...");
                match upload::image(data) {
                    Ok(_) => log::info!("Upload successful!"),
                    Err(e) => log::error!("Upload failed: {:?}", e),
                }
                log::info!("Background thread: ready for next task");
            }
        })?;

    loop {
        let is_pressed = button.is_low();

        if waiting_for_release {
            if !is_pressed {
                waiting_for_release = false;
                log::info!("Wake button released; button input ready");
            }
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        if is_pressed != prev_is_pressed {
            if is_pressed {
                log::info!("👇 按钮被按下了！ (Pressed)");
                pressed_at = Some(Instant::now());
                is_long_press = false;
            } else if is_long_press
                || pressed_at
                    .is_some_and(|started| started.elapsed() >= Duration::from_millis(2500))
            {
                log::info!("Long press -> deep sleep");
                esp_idf_sys::esp!(unsafe {
                    esp_idf_sys::esp_sleep_pd_config(
                        esp_idf_sys::esp_sleep_pd_domain_t_ESP_PD_DOMAIN_RTC_PERIPH,
                        esp_idf_sys::esp_sleep_pd_option_t_ESP_PD_OPTION_ON,
                    )
                })?;
                esp_idf_sys::esp!(unsafe {
                    esp_idf_sys::esp_sleep_enable_ext1_wakeup(
                        1_u64 << 2,
                        esp_idf_sys::esp_sleep_ext1_wakeup_mode_t_ESP_EXT1_WAKEUP_ANY_LOW,
                    )
                })?;
                unsafe { esp_idf_sys::esp_deep_sleep_start() };
            } else if pressed_at.is_some() {
                log::info!("Short press -> capture");
                let free = unsafe { esp_idf_sys::esp_get_free_heap_size() };
                log::info!("Current Free Heap: {} bytes", free);

                if let Some(frame) = camera::CameraFrame::get() {
                    let data = frame.data().to_vec();
                    log::info!("Picture taken! Size: {} bytes. Uploading...", data.len());
                    tx.send(data).unwrap()
                } else {
                    log::error!("Camera Capture Failed");
                }
            }
            prev_is_pressed = is_pressed;
        }

        if is_pressed
            && !is_long_press
            && pressed_at.is_some_and(|started| started.elapsed() >= Duration::from_millis(2500))
        {
            is_long_press = true;
            log::info!("Long press detected; release button to enter deep sleep");
        }

        thread::sleep(Duration::from_millis(50));
    }
}
