//! A super simple peak meter widget.

use crossbeam::atomic::AtomicCell;
use iced_baseview::core::widget::Tree;
use std::marker::PhantomData;
use std::time::Duration;
use std::time::Instant;

use crate::core::{
    alignment, layout, mouse, renderer, text, Background, Color, Element, Layout, Length,
    Rectangle, Size, Widget,
};

use crate::core::text::Renderer as TextRenderer;
use crate::core::widget::text::StyleSheet as TextStyleSheet;

/// The thickness of this widget's borders.
const BORDER_WIDTH: f32 = 1.0;
/// The thickness of a tick inside of the peak meter's bar.
const TICK_WIDTH: f32 = 1.0;

/// A simple horizontal peak meter.
///
/// TODO: There are currently no styling options at all
/// TODO: Vertical peak meter, this is just a proof of concept to fit the gain GUI example.
pub struct PeakMeter<Message, Renderer> 
where 
    Renderer: TextRenderer,
    Renderer::Theme: TextStyleSheet
{
    state: State,

    /// The current measured value in decibel.
    current_value_db: f32,

    /// The time the old peak value should remain visible.
    hold_time: Option<Duration>,

    height: Length,
    width: Length,
    text_size: Option<f32>,
    font: Option<Renderer::Font>,

    /// We don't emit any messages, but iced requires us to define some message type anyways.
    _message: PhantomData<Message>,
    _renderer: PhantomData<Renderer>,
}

/// State for a [`PeakMeter`].
#[derive(Debug, Default)]
pub struct State {
    /// The last peak value in decibel.
    held_peak_value_db: AtomicCell<f32>,
    /// When the last peak value was hit.
    last_held_peak_value: AtomicCell<Option<Instant>>,
}

impl<'a, Message, Renderer> PeakMeter<Message, Renderer> 
where 
    Renderer: TextRenderer,
    Renderer::Theme: TextStyleSheet
{
    /// Creates a new [`PeakMeter`] using the current measurement in decibel. This measurement can
    /// already have some form of smoothing applied to it. This peak slider widget can draw the last
    /// hold value for you.
    pub fn new(value_db: f32) -> Self {
        Self {
            state: Default::default(),

            current_value_db: value_db,

            hold_time: None,

            width: Length::Fixed(180.0),
            height: Length::Fixed(30.0),
            text_size: None,
            font: None,

            _message: PhantomData,
            _renderer: PhantomData
        }
    }

    /// Keep showing the peak value for a certain amount of time.
    pub fn hold_time(mut self, time: Duration) -> Self {
        self.hold_time = Some(time);
        self
    }

    /// Sets the width of the [`PeakMeter`].
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Sets the height of the [`PeakMeter`].
    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    /// Sets the text size of the [`PeakMeter`]'s ticks bar.
    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = Some(size);
        self
    }

    /// Sets the font of the [`PeakMeter`]'s ticks bar.
    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for PeakMeter<Message, Renderer>
where
    Message: Clone,
    Renderer: TextRenderer,
    Renderer::Theme: TextStyleSheet
{
    fn width(&self) -> Length {
        self.width
    }

    fn height(&self) -> Length {
        self.height
    }

    fn layout(&self, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);
        let size = limits.resolve(Size::ZERO);

        layout::Node::new(size)
    }

    fn draw(
        &self,
        _state: &Tree,
        renderer: &mut Renderer,
        _theme: &Renderer::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor_position: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let bar_bounds = Rectangle {
            height: bounds.height / 2.0,
            ..bounds
        };
        let ticks_bounds = Rectangle {
            y: bounds.y + (bounds.height / 2.0),
            height: bounds.height / 2.0,
            ..bounds
        };

        let text_size = self
            .text_size
            .unwrap_or_else(|| (renderer.default_size() as f32 * 0.7).round());

        // We'll draw a simple horizontal for [-90, 20] dB where we'll treat -80 as -infinity, with
        // a label containing the tick markers below it. If `.hold_time()` was called then we'll
        // also display the last held value
        const MIN_TICK: f32 = -90.0;
        const MAX_TICK: f32 = 20.0;
        let text_ticks = [-80i32, -60, -40, -20, 0];
        // Draw a tick with one pixel in between, otherwise the bilinear interpolation makes
        // everything a smeary mess
        let bar_ticks_start = (bar_bounds.x + BORDER_WIDTH).round() as i32;
        let bar_ticks_end = (bar_bounds.x + bar_bounds.width - (BORDER_WIDTH * 2.0)).ceil() as i32;
        let bar_tick_coordinates =
            (bar_ticks_start..bar_ticks_end).step_by((TICK_WIDTH + 1.0).round() as usize);
        let db_to_x_coord = |db: f32| {
            let tick_fraction = (db - MIN_TICK) / (MAX_TICK - MIN_TICK);
            bar_ticks_start as f32
                + ((bar_ticks_end - bar_ticks_start) as f32 * tick_fraction).round()
        };

        for tick_x in bar_tick_coordinates {
            let tick_fraction =
                (tick_x - bar_ticks_start) as f32 / (bar_ticks_end - bar_ticks_start) as f32;
            let tick_db = (tick_fraction * (MAX_TICK - MIN_TICK)) + MIN_TICK;
            if tick_db > self.current_value_db {
                break;
            }

            let tick_bounds = Rectangle {
                x: tick_x as f32,
                y: bar_bounds.y + BORDER_WIDTH,
                width: TICK_WIDTH,
                height: bar_bounds.height - (BORDER_WIDTH * 2.0),
            };

            let grayscale_color = 0.3 + ((1.0 - tick_fraction) * 0.5);
            let tick_color = Color::from_rgb(grayscale_color, grayscale_color, grayscale_color);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: tick_bounds,
                    border_radius: [0.0; 4].into(),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
                Background::Color(tick_color),
            );
        }

        // Draw the hold peak value if the hold time option has been set
        if let Some(hold_time) = self.hold_time {
            let now = Instant::now();
            let mut held_peak_value_db = self.state.held_peak_value_db.load();
            let last_peak_value = self.state.last_held_peak_value.load();
            if self.current_value_db >= held_peak_value_db
                || last_peak_value.is_none()
                || now > last_peak_value.unwrap() + hold_time
            {
                self.state.held_peak_value_db.store(self.current_value_db);
                self.state.last_held_peak_value.store(Some(now));
                held_peak_value_db = self.current_value_db;
            }

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: db_to_x_coord(held_peak_value_db),
                        y: bar_bounds.y + BORDER_WIDTH,
                        width: TICK_WIDTH,
                        height: bar_bounds.height - (BORDER_WIDTH * 2.0),
                    },
                    border_radius: [0.0; 4].into(),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
                Background::Color(Color::from_rgb(0.3, 0.3, 0.3)),
            );
        }

        // Draw the bar after the ticks since the first and last tick may overlap with the borders
        renderer.fill_quad(
            renderer::Quad {
                bounds: bar_bounds,
                border_radius: [0.0; 4].into(),
                border_width: BORDER_WIDTH,
                border_color: Color::BLACK,
            },
            Background::Color(Color::TRANSPARENT),
        );

        let font = self.font.unwrap_or_else(|| renderer.default_font());
        // Beneath the bar we want to draw the names of the ticks
        for tick_db in text_ticks {
            let x_coordinate = db_to_x_coord(tick_db as f32);

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: x_coordinate,
                        y: ticks_bounds.y,
                        width: TICK_WIDTH,
                        height: ticks_bounds.height * 0.3,
                    },
                    border_radius: [0.0; 4].into(),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
                Background::Color(Color::from_rgb(0.3, 0.3, 0.3)),
            );

            let tick_text = if tick_db == text_ticks[0] {
                String::from("-inf")
            } else {
                tick_db.to_string()
            };
            renderer.fill_text(text::Text {
                content: &tick_text,
                font,
                size: text_size as f32,
                bounds: Rectangle {
                    x: x_coordinate,
                    y: ticks_bounds.y + (ticks_bounds.height * 0.35),
                    ..ticks_bounds
                },
                color: style.text_color,
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Top,
                line_height: text::LineHeight::default(),
                shaping: text::Shaping::Basic,
            });
        }

        // Every proper graph needs a unit label
        let zero_db_x_coordinate = db_to_x_coord(0.0);
        let zero_db_text_width = renderer.measure_width(
            "0", 
            text_size, 
            font,
            text::Shaping::Basic,
        );
        renderer.fill_text(text::Text {
            // The spacing looks a bit off if we start with a space here so we'll add a little
            // offset to the x-coordinate instead
            content: "dBFS",
            font,
            size: text_size as f32,
            bounds: Rectangle {
                x: zero_db_x_coordinate + (zero_db_text_width / 2.0) + (text_size as f32 * 0.2),
                y: ticks_bounds.y + (ticks_bounds.height * 0.35),
                ..ticks_bounds
            },
            color: style.text_color,
            horizontal_alignment: alignment::Horizontal::Left,
            vertical_alignment: alignment::Vertical::Top,
            line_height: text::LineHeight::default(),
            shaping: text::Shaping::Basic,
        });
    }
}

impl<'a, Message, Renderer> From<PeakMeter<Message, Renderer>> for Element<'a, Message, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + TextRenderer,
    Renderer::Theme: TextStyleSheet
{
    fn from(widget: PeakMeter<Message, Renderer>) -> Self {
        Element::new(widget)
    }
}
