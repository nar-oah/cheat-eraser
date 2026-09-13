use esp_idf_svc::hal::gpio::{PinDriver, Pull};
use esp_idf_svc::hal::prelude::*;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
mod camera;
mod upload;
mod wifi;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;

    let _wifi = wifi::connect(peripherals.modem)?;
    camera::init()?;

    let mut button = PinDriver::input(peripherals.pins.gpio2)?;
    button.set_pull(Pull::Up)?;
    let mut prev_is_pressed = false;

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
        if is_pressed != prev_is_pressed {
            if is_pressed {
                log::info!("👇 按钮被按下了！ (Pressed)");
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
        thread::sleep(Duration::from_millis(50));
    }
}
