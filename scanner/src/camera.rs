use anyhow::{anyhow, Result};
use esp_idf_svc::sys;
use esp_idf_sys::camera;

pub struct CameraFrame {
    fb: *mut camera::camera_fb_t,
}
impl CameraFrame {
    pub fn get() -> Option<Self> {
        let fb = unsafe { camera::esp_camera_fb_get() };
        if fb.is_null() {
            None
        } else {
            Some(Self { fb })
        }
    }
    pub fn data(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts((*self.fb).buf, (*self.fb).len) }
    }
}
impl Drop for CameraFrame {
    fn drop(&mut self) {
        unsafe { camera::esp_camera_fb_return(self.fb) };
    }
}
pub fn init() -> Result<()> {
    let config = camera::camera_config_t {
        pin_pwdn: -1,
        pin_reset: -1,
        pin_xclk: 10,
        __bindgen_anon_1: camera::camera_config_t__bindgen_ty_1 { pin_sccb_sda: 40 },
        __bindgen_anon_2: camera::camera_config_t__bindgen_ty_2 { pin_sccb_scl: 39 },

        pin_d7: 48,
        pin_d6: 11,
        pin_d5: 12,
        pin_d4: 14,
        pin_d3: 16,
        pin_d2: 18,
        pin_d1: 17,
        pin_d0: 15,
        pin_vsync: 38,
        pin_href: 47,
        pin_pclk: 13,

        xclk_freq_hz: 20_000_000,
        ledc_timer: sys::ledc_timer_t_LEDC_TIMER_0,
        ledc_channel: sys::ledc_channel_t_LEDC_CHANNEL_0,

        pixel_format: camera::pixformat_t_PIXFORMAT_JPEG,
        frame_size: camera::framesize_t_FRAMESIZE_QXGA,

        jpeg_quality: 6,
        fb_count: 1,
        fb_location: camera::camera_fb_location_t_CAMERA_FB_IN_PSRAM,
        grab_mode: camera::camera_grab_mode_t_CAMERA_GRAB_LATEST,
        ..Default::default()
    };
    let err = unsafe { camera::esp_camera_init(&config) };
    if err != 0 {
        return Err(anyhow!("Camera init failed with error code: {}", err));
    }
    let sensor = unsafe { camera::esp_camera_sensor_get() };
    let pid = unsafe { (*sensor).id.PID };
    let model = match pid {
        0x26 => "OV2640",
        0x3660 => "OV3660",
        0x5640 => "OV5640",
        _ => "Unknown",
    };
    log::info!("Camera detected: PID=0x{:04X}, model={}", pid, model);
    Ok(())
}
