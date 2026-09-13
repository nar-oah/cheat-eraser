use esp_idf_svc::hal::gpio::{Input, InputPin, OutputPin, PinDriver, Pull};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonEvent {
    Previous,
    Next,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonEdge {
    Pressed,
    Released,
}

pub struct ControlButton<'d, T: InputPin> {
    button: PinDriver<'d, T, Input>,
    state: bool,
}

impl<'d, T: InputPin + OutputPin> ControlButton<'d, T> {
    pub fn init(pin: T) -> anyhow::Result<Self> {
        let button = PinDriver::input(pin, Pull::Up)?;
        let state = button.is_low();
        Ok(Self { button, state })
    }
    fn poll(&mut self) -> Option<ButtonEdge> {
        let is_pressed: bool = self.button.is_low();
        if is_pressed != self.state {
            self.state = is_pressed;
            return Some(if is_pressed {
                ButtonEdge::Pressed
            } else {
                ButtonEdge::Released
            });
        }
        None
    }
    fn is_pressed(&self) -> bool {
        self.state
    }
}

pub struct ButtonController<'d, L: InputPin, R: InputPin> {
    left: ControlButton<'d, L>,
    right: ControlButton<'d, R>,
    chord: bool,
}

impl<'d, L: InputPin + OutputPin, R: InputPin + OutputPin> ButtonController<'d, L, R> {
    pub fn init(left: L, right: R) -> anyhow::Result<Self> {
        Ok(Self {
            left: ControlButton::init(left)?,
            right: ControlButton::init(right)?,
            chord: false,
        })
    }
    pub fn poll(&mut self) -> Option<ButtonEvent> {
        let left_edge = self.left.poll();
        let right_edge = self.right.poll();
        let left_pressed = self.left.is_pressed();
        let right_pressed = self.right.is_pressed();

        self.chord = self.chord || (left_pressed && right_pressed);
        if self.chord && !left_pressed && !right_pressed {
            self.chord = false;
            return Some(ButtonEvent::Reset);
        }
        if self.chord {
            return None;
        }
        match (left_edge, right_edge) {
            (Some(ButtonEdge::Released), _) => Some(ButtonEvent::Previous),
            (_, Some(ButtonEdge::Released)) => Some(ButtonEvent::Next),
            _ => None,
        }
    }
}
