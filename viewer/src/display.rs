use anyhow::Result;
use embedded_graphics::{
    image::Image,
    prelude::{DrawTarget, Drawable, OriginDimensions, Point, Primitive},
    primitives::{Line, PrimitiveStyle},
};
use epd_waveshare::{
    color::Color,
    epd1in54_v2::{Display1in54, Epd1in54},
    prelude::WaveshareDisplay,
};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyIOPin, AnyInputPin, AnyOutputPin, Input, Output, PinDriver, Pull},
    peripheral::Peripheral,
    spi::{config::Config, SpiAnyPins, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
    units::FromValueType,
};
use std::time::{Duration, Instant};
use tinybmp::Bmp;
use u8g2_fonts::{fonts, U8g2TextStyle};

const BACKGROUND: Color = Color::Black;
const COLOR: Color = Color::White;
pub const BORDER: u32 = 3;
const REFRESH_THRESHOLD: u8 = 20;

pub struct ScenePins {
    pub sck: AnyOutputPin,
    pub mosi: AnyOutputPin,
    pub cs: AnyOutputPin,
    pub dc: AnyOutputPin,
    pub rst: AnyOutputPin,
    pub busy: AnyInputPin,
}

pub struct Scene<'a> {
    spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
    epd: Epd1in54<
        SpiDeviceDriver<'a, SpiDriver<'a>>,
        PinDriver<'a, AnyInputPin, Input>,
        PinDriver<'a, AnyOutputPin, Output>,
        PinDriver<'a, AnyOutputPin, Output>,
        FreeRtos,
    >,
    display: Display1in54,
    text_style: U8g2TextStyle<Color>,
    is_sleep: bool,
    active_time: Instant,
    refresh_count: u8,
}

impl<'a> Scene<'a> {
    pub fn new<S: SpiAnyPins>(p_spi: impl Peripheral<P = S> + 'a, pins: ScenePins) -> Result<Self> {
        let dc = PinDriver::output(pins.dc)?;
        let rst = PinDriver::output(pins.rst)?;
        let busy = PinDriver::input(pins.busy, Pull::Floating)?;
        let mut spi = SpiDeviceDriver::new_single(
            p_spi,
            pins.sck,
            pins.mosi,
            Option::<AnyIOPin>::None,
            Some(pins.cs),
            &SpiDriverConfig::new(),
            &Config::new().baudrate(2.MHz().into()),
        )?;
        let epd = Epd1in54::new(&mut spi, busy, dc, rst, &mut FreeRtos, Some(10))?;
        return Ok(Self {
            spi,
            epd,
            display: Display1in54::default(),
            text_style: U8g2TextStyle::new(fonts::u8g2_font_wqy12_t_gb2312, COLOR),
            is_sleep: false,
            active_time: Instant::now(),
            refresh_count: REFRESH_THRESHOLD,
        });
    }
    pub fn add_line(&mut self, start: Point, end: Point) -> Result<()> {
        Line::new(start, end)
            .into_styled(PrimitiveStyle::with_stroke(COLOR, BORDER))
            .draw(&mut self.display)?;
        Ok(())
    }
    pub fn add_text(&mut self, text: &str, point: Point) -> Result<()> {
        let style = self.text_style.clone();
        embedded_graphics::text::Text::new(&text, point, style).draw(&mut self.display)?;
        Ok(())
    }
    pub fn mod_logo(&mut self, bmp_data: Vec<u8>) -> Result<()> {
        let bmp = Bmp::from_slice(&bmp_data).unwrap();
        let width = bmp.size().width as i32;
        let height = bmp.size().height as i32;
        let x = (200 - width) / 2;
        let y = (67 - height) / 2;
        Image::new(&bmp, Point::new(x, y)).draw(&mut self.display)?;
        Ok(())
    }
    pub fn check_wakeup(&mut self) -> Result<()> {
        if self.is_sleep {
            self.epd.wake_up(&mut self.spi, &mut FreeRtos)?;
            self.is_sleep = false
        }
        self.active_time = Instant::now();
        Ok(())
    }
    pub fn check_sleep(&mut self) -> Result<()> {
        if !self.is_sleep && self.active_time.elapsed() > Duration::from_secs(60) {
            self.epd.sleep(&mut self.spi, &mut FreeRtos)?;
            self.is_sleep = true
        }
        Ok(())
    }
    pub fn refresh(&mut self) -> Result<()> {
        self.check_wakeup()?;
        if self.refresh_count >= REFRESH_THRESHOLD {
            self.epd.update_and_display_frame(
                &mut self.spi,
                self.display.buffer(),
                &mut FreeRtos,
            )?;
            self.refresh_count = 0;
        } else {
            self.epd.update_partial_frame(
                &mut self.spi,
                &mut FreeRtos,
                self.display.buffer(),
                0,
                0,
                200,
                200,
            )?;
            self.epd.display_frame(&mut self.spi, &mut FreeRtos)?;
            self.refresh_count += 1;
        }
        Ok(())
    }
    pub fn clear(&mut self) -> Result<()> {
        self.display.clear(BACKGROUND)?;
        Ok(())
    }
}
