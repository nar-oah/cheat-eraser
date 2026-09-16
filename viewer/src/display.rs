use anyhow::{anyhow, Result};
use embedded_graphics::{
    image::Image,
    prelude::{DrawTarget, Drawable, OriginDimensions, Point, Primitive},
    primitives::{Line, PrimitiveStyle},
};
use epd_waveshare::{
    color::Color,
    epd1in54_v2::{Display1in54, Epd1in54},
    prelude::{RefreshLut, WaveshareDisplay},
};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyIOPin, AnyInputPin, AnyOutputPin, Input, Output, PinDriver, Pull},
    spi::{config::Config, SpiAnyPins, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
    units::FromValueType,
};
use std::time::{Duration, Instant};
use tinybmp::Bmp;
use u8g2_fonts::{fonts, U8g2TextStyle};

const BACKGROUND: Color = Color::Black;
const COLOR: Color = Color::White;
pub const BORDER: u32 = 3;

pub struct ScenePins<'a> {
    pub sck: AnyOutputPin<'a>,
    pub mosi: AnyOutputPin<'a>,
    pub cs: AnyOutputPin<'a>,
    pub dc: AnyOutputPin<'a>,
    pub rst: AnyOutputPin<'a>,
    pub busy: AnyInputPin<'a>,
}

pub struct Scene<'a> {
    spi: SpiDeviceDriver<'a, SpiDriver<'a>>,
    epd: Epd1in54<
        SpiDeviceDriver<'a, SpiDriver<'a>>,
        PinDriver<'a, Input>,
        PinDriver<'a, Output>,
        PinDriver<'a, Output>,
        FreeRtos,
    >,
    display: Display1in54,
    text_style: U8g2TextStyle<Color>,
    is_sleep: bool,
    active_time: Instant,
    refresh_lut: RefreshLut,
}

impl<'a> Scene<'a> {
    pub fn new<S: SpiAnyPins + 'a>(p_spi: S, pins: ScenePins<'a>) -> Result<Self> {
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
        Ok(Self {
            spi,
            epd,
            display: Display1in54::default(),
            text_style: U8g2TextStyle::new(fonts::u8g2_font_wqy12_t_gb2312, COLOR),
            is_sleep: false,
            active_time: Instant::now(),
            refresh_lut: RefreshLut::Full,
        })
    }
    pub fn add_line(&mut self, start: Point, end: Point) -> Result<()> {
        Line::new(start, end)
            .into_styled(PrimitiveStyle::with_stroke(COLOR, BORDER))
            .draw(&mut self.display)?;
        Ok(())
    }
    pub fn add_text(&mut self, text: &str, point: Point) -> Result<()> {
        let style = self.text_style.clone();
        embedded_graphics::text::Text::new(text, point, style).draw(&mut self.display)?;
        Ok(())
    }
    pub fn mod_logo(&mut self, bmp_data: Vec<u8>) -> Result<()> {
        let bmp = Bmp::from_slice(&bmp_data)
            .map_err(|error| anyhow!("Invalid BMP from HTTP endpoint /formula: {error:?}"))?;
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
        if self.refresh_lut != RefreshLut::Quick {
            self.epd
                .set_lut(&mut self.spi, &mut FreeRtos, Some(RefreshLut::Quick))?;
            self.refresh_lut = RefreshLut::Quick;
        }
        self.epd
            .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut FreeRtos)?;
        Ok(())
    }
    pub fn full_refresh(&mut self) -> Result<()> {
        self.check_wakeup()?;
        if self.refresh_lut != RefreshLut::Full {
            self.epd
                .set_lut(&mut self.spi, &mut FreeRtos, Some(RefreshLut::Full))?;
            self.refresh_lut = RefreshLut::Full;
        }
        self.epd
            .update_and_display_frame(&mut self.spi, self.display.buffer(), &mut FreeRtos)?;
        Ok(())
    }
    pub fn clear(&mut self) -> Result<()> {
        self.display.clear(BACKGROUND)?;
        Ok(())
    }
}
