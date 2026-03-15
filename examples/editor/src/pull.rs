//! Drag-to-adjust ("pull") controls — click and drag an icon label to
//! continuously adjust a numeric value along one axis.

use iced::{Event, Point, Subscription, event, mouse};

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    LetterSpacing,
    LineHeight,
}

#[derive(Debug, Clone)]
pub enum Message {
    Start(Kind),
    Move(Point),
    End,
}

/// Configuration for a pull-to-adjust control.
pub struct Config {
    pub axis: Axis,
    /// Base value change per effective pixel of drag.
    pub sensitivity: f32,
    /// Rounding precision (e.g. 0.01).
    pub precision: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// An active pull gesture — at most one can be active at a time.
#[derive(Debug, Clone)]
pub enum Pull {
    LetterSpacing(State),
    LineHeight(State),
}

/// Internals of an active pull.
#[derive(Debug, Clone)]
pub struct State {
    start_value: f32,
    origin: Option<f32>,
}

impl Pull {
    pub fn letter_spacing(start_value: f32) -> Self {
        Self::LetterSpacing(State {
            start_value,
            origin: None,
        })
    }

    pub fn line_height(start_value: f32) -> Self {
        Self::LineHeight(State {
            start_value,
            origin: None,
        })
    }

    fn state_and_config(&mut self) -> (&mut State, &'static Config) {
        match self {
            Self::LetterSpacing(s) => (s, &LETTER_SPACING),
            Self::LineHeight(s) => (s, &LINE_HEIGHT),
        }
    }

    /// Process a global cursor move. Returns the new value.
    pub fn moved(&mut self, position: Point) -> f32 {
        let (state, config) = self.state_and_config();

        let coord = match config.axis {
            Axis::Horizontal => position.x,
            Axis::Vertical => position.y,
        };

        let origin = match state.origin {
            Some(o) => o,
            None => {
                state.origin = Some(coord);
                return state.start_value;
            }
        };

        let delta_px = match config.axis {
            Axis::Horizontal => coord - origin,
            Axis::Vertical => -(coord - origin),
        };

        let raw = state.start_value + delta_px * config.sensitivity;
        let inv = (1.0 / config.precision).round();
        let rounded = (raw * inv).round() / inv;
        rounded.clamp(config.min, config.max)
    }
}

/// Subscribe to global cursor events while a pull is active.
pub fn subscription(pull: &Option<Pull>) -> Subscription<Message> {
    if pull.is_some() {
        event::listen_with(|event, _, _| match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Message::Move(position)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Message::End),
            _ => None,
        })
    } else {
        Subscription::none()
    }
}

pub const LETTER_SPACING: Config = Config {
    axis: Axis::Horizontal,
    sensitivity: 0.01,
    precision: 0.01,
    min: -0.2,
    max: 0.2,
};

pub const LINE_HEIGHT: Config = Config {
    axis: Axis::Vertical,
    sensitivity: 0.1,
    precision: 0.1,
    min: 0.5,
    max: 5.0,
};
