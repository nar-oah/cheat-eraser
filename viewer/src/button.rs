use esp_idf_svc::hal::gpio::{Input, InputPin, OutputPin, PinDriver, Pull};
use std::time::{Duration, Instant};

const LONG_PRESS_DURATION: Duration = Duration::from_millis(2500);
const RELEASE_STABLE_DURATION: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonEvent {
    Previous,
    Next,
    Refresh,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonEdge {
    Pressed,
    Released,
}

pub struct ControlButton<'d> {
    button: PinDriver<'d, Input>,
    state: bool,
}

impl<'d> ControlButton<'d> {
    pub fn init<T: InputPin + OutputPin + 'd>(pin: T) -> anyhow::Result<Self> {
        let button = PinDriver::input(pin, Pull::Up)?;
        let state = button.is_low();
        Ok(Self { button, state })
    }
    fn poll(&mut self) -> Option<ButtonEdge> {
        let is_pressed = self.button.is_low();
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

pub struct ButtonController<'d> {
    left: ControlButton<'d>,
    right: ControlButton<'d>,
    left_pressed_at: Option<Instant>,
    right_pressed_at: Option<Instant>,
    chord: bool,
    pending_event: Option<ButtonEvent>,
    waiting_for_release: bool,
    released_at: Option<Instant>,
}

impl<'d> ButtonController<'d> {
    pub fn init<L: InputPin + OutputPin + 'd, R: InputPin + OutputPin + 'd>(
        left: L,
        right: R,
        waiting_for_release: bool,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            left: ControlButton::init(left)?,
            right: ControlButton::init(right)?,
            left_pressed_at: None,
            right_pressed_at: None,
            chord: false,
            pending_event: None,
            waiting_for_release,
            released_at: None,
        })
    }

    pub fn poll(&mut self) -> Option<ButtonEvent> {
        let left_edge = self.left.poll();
        let right_edge = self.right.poll();
        let left_pressed = self.left.is_pressed();
        let right_pressed = self.right.is_pressed();

        if matches!(left_edge, Some(ButtonEdge::Pressed)) {
            self.left_pressed_at = Some(Instant::now());
        }
        if matches!(right_edge, Some(ButtonEdge::Pressed)) {
            self.right_pressed_at = Some(Instant::now());
        }

        if self.waiting_for_release {
            if self.release_is_stable(!left_pressed && !right_pressed) {
                self.waiting_for_release = false;
                self.clear_press_state();
                log::info!("Wake button released; button input ready");
            }
            return None;
        }

        self.chord |= left_pressed && right_pressed;

        let left_long = self.left_pressed_at.is_some_and(|started| {
            started.elapsed() >= LONG_PRESS_DURATION
                && (left_pressed || matches!(left_edge, Some(ButtonEdge::Released)))
        });
        let right_long = self.right_pressed_at.is_some_and(|started| {
            started.elapsed() >= LONG_PRESS_DURATION
                && (right_pressed || matches!(right_edge, Some(ButtonEdge::Released)))
        });

        if self.pending_event.is_none() {
            self.pending_event = if left_long {
                Some(ButtonEvent::Shutdown)
            } else if right_long {
                Some(ButtonEvent::Refresh)
            } else {
                None
            };
            if let Some(event) = self.pending_event {
                log::info!("Long press detected; release button(s) for {:?}", event);
            }
        }

        if let Some(event) = self.pending_event {
            if self.release_is_stable(!left_pressed && !right_pressed) {
                self.clear_press_state();
                return Some(event);
            }
            return None;
        }

        if self.chord {
            if !left_pressed && !right_pressed {
                self.clear_press_state();
            }
            return None;
        }

        match (left_edge, right_edge) {
            (Some(ButtonEdge::Released), _) => {
                self.left_pressed_at = None;
                Some(ButtonEvent::Previous)
            }
            (_, Some(ButtonEdge::Released)) => {
                self.right_pressed_at = None;
                Some(ButtonEvent::Next)
            }
            _ => None,
        }
    }

    fn release_is_stable(&mut self, all_released: bool) -> bool {
        if !all_released {
            self.released_at = None;
            return false;
        }
        self.released_at.get_or_insert_with(Instant::now).elapsed() >= RELEASE_STABLE_DURATION
    }

    fn clear_press_state(&mut self) {
        self.left_pressed_at = None;
        self.right_pressed_at = None;
        self.chord = false;
        self.pending_event = None;
        self.released_at = None;
    }
}
