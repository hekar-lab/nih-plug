//! A slider that integrates with NIH-plug's [`Param`] types.

use atomic_refcell::AtomicRefCell;
use iced_baseview::core::widget::Tree;
use iced_baseview::core::BorderRadius;
use nih_plug::prelude::Param;
use std::borrow::Borrow;
use std::marker::PhantomData;

use crate::core::widget::tree;
use crate::core::{
    alignment, event, keyboard, layout, mouse, renderer, text, touch, Background, Clipboard, Color,
    Element, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget,
};
use crate::widget::{self, TextInput, text_input::StyleSheet as TextInputStyleSheet};
use crate::core::widget::text::StyleSheet as TextStyleSheet;
use crate::core::mouse::Cursor;

use crate::core::text::Renderer as TextRenderer;

use super::util;
use super::ParamMessage;

/// When shift+dragging a parameter, one pixel dragged corresponds to this much change in the
/// noramlized parameter.
const GRANULAR_DRAG_MULTIPLIER: f32 = 0.1;

/// The thickness of this widget's borders.
const BORDER_WIDTH: f32 = 1.0;

/// A slider that integrates with NIH-plug's [`Param`] types.
///
/// TODO: There are currently no styling options at all
/// TODO: Handle scrolling for steps (and shift+scroll for smaller steps?)
pub struct ParamSlider<'a, P: Param, Renderer>
where
    Renderer: TextRenderer,
    Renderer::Theme: TextInputStyleSheet + TextStyleSheet,
{
    param: &'a P,

    height: Length,
    width: Length,
    text_size: Option<f32>,
    font: Option<Renderer::Font>,

    _renderer: PhantomData<Renderer>,
}

/// State for a [`ParamSlider`].
#[derive(Debug, Default)]
pub struct State {
    keyboard_modifiers: keyboard::Modifiers,
    /// Will be set to `true` if we're dragging the parameter. Resetting the parameter or entering a
    /// text value should not initiate a drag.
    drag_active: bool,
    /// We keep track of the start coordinate and normalized value holding down Shift while dragging
    /// for higher precision dragging. This is a `None` value when granular dragging is not active.
    granular_drag_start_x_value: Option<(f32, f32)>,
    /// Track clicks for double clicks.
    last_click: Option<mouse::Click>,

    /// State for the text input overlay that will be shown when this widget is alt+clicked.
    text_input_state: AtomicRefCell<widget::text_input::State>,
    /// The text that's currently in the text input. If this is set to `None`, then the text input
    /// is not visible.
    text_input_value: Option<String>,
}

impl State {
    pub fn new() -> Self {
        State::default()
    }
}

/// An internal message for intercep- I mean handling output from the embedded [`TextInpu`] widget.
#[derive(Debug, Clone)]
enum TextInputMessage {
    /// A new value was entered in the text input dialog.
    Value(String),
    /// Enter was pressed.
    Submit,
}

/// The default text input style with the border removed.
struct TextInputStyle;

impl TextInputStyleSheet for TextInputStyle {
    type Style = ();

    fn active(&self, _style: &Self::Style) -> widget::text_input::Appearance {
        widget::text_input::Appearance {
            background: Background::Color(Color::TRANSPARENT),
            border_radius: BorderRadius::default(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            icon_color: Color::BLACK,
        }
    }

    fn focused(&self, style: &Self::Style) -> widget::text_input::Appearance {
        self.active(style)
    }

    fn placeholder_color(&self, _style: &Self::Style) -> Color {
        Color::from_rgb(0.7, 0.7, 0.7)
    }

    fn value_color(&self, _style: &Self::Style) -> Color {
        Color::from_rgb(0.3, 0.3, 0.3)
    }

    fn selection_color(&self, _style: &Self::Style) -> Color {
        Color::from_rgb(0.8, 0.8, 1.0)
    }
    
    fn disabled_color(&self, _style: &Self::Style) -> Color {
        Color::from_rgb(0.5, 0.5, 0.5)
    }
    
    fn disabled(&self, _style: &Self::Style) -> widget::text_input::Appearance {
        widget::text_input::Appearance {
            background: Background::Color(Color::TRANSPARENT),
            border_radius: BorderRadius::default(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            icon_color: Color::BLACK,
        }
    }
}

impl<'a, P: Param, Renderer> ParamSlider<'a, P, Renderer> 
where
    Renderer: TextRenderer,
    Renderer::Theme: TextInputStyleSheet + TextStyleSheet,
{
    /// Creates a new [`ParamSlider`] for the given parameter.
    pub fn new(param: &'a P) -> Self {
        Self {
            param,

            width: Length::Fixed(180.0),
            height: Length::Fixed(30.0),
            text_size: None,
            font: None,

            _renderer: PhantomData
        }
    }

    /// Sets the width of the [`ParamSlider`].
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Sets the height of the [`ParamSlider`].
    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    /// Sets the text size of the [`ParamSlider`].
    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = Some(size);
        self
    }

    /// Sets the font of the [`ParamSlider`].
    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Create a temporary [`TextInput`] hooked up to [`State::text_input_value`] and outputting
    /// [`TextInputMessage`] messages and do something with it. This can be used to
    fn with_text_input<O, R, F>(&self, state: &State, layout: Layout, renderer: R, current_value: &str, f: F) -> O
    where
        F: FnOnce(TextInput<'_, TextInputMessage, Renderer>, Layout, R) -> O,
        R: Borrow<Renderer>,
    {
        let mut text_input_state = state.text_input_state.borrow_mut();
        text_input_state.focus();

        let text_size = self
            .text_size
            .unwrap_or_else(|| renderer.borrow().default_size());

        let font = self.font.unwrap_or_else(|| renderer.borrow().default_font());
        let text_width = renderer
            .borrow()
            .measure_width(current_value, text_size, font, text::Shaping::Basic);

        let text_input = TextInput::<'a, _, Renderer>::new(
            "",
            current_value,
        )
        .font(font)
        .size(text_size)
        .width(Length::Fixed(text_width.ceil()))
        //.style(TextInputStyle)
        .on_submit(TextInputMessage::Submit);

        // Make sure to not draw over the borders, and center the text
        let offset_node = layout::Node::with_children(
            Size {
                width: text_width,
                height: layout.bounds().size().height - (BORDER_WIDTH * 2.0),
            },
            vec![layout::Node::new(layout.bounds().size())],
        );
        let offset_layout = Layout::with_offset(
            Vector {
                x: layout.bounds().center_x() - (text_width / 2.0),
                y: layout.position().y + BORDER_WIDTH,
            },
            &offset_node,
        );

        f(text_input, offset_layout, renderer)
    }

    /// Set the normalized value for a parameter if that would change the parameter's plain value
    /// (to avoid unnecessary duplicate parameter changes). The begin- and end set parameter
    /// messages need to be sent before calling this function.
    fn set_normalized_value(&self, shell: &mut Shell<'_, ParamMessage>, normalized_value: f32) {
        // This snaps to the nearest plain value if the parameter is stepped in some way.
        // TODO: As an optimization, we could add a `const CONTINUOUS: bool` to the parameter to
        //       avoid this normalized->plain->normalized conversion for parameters that don't need
        //       it
        let plain_value = self.param.preview_plain(normalized_value);
        let current_plain_value = self.param.modulated_plain_value();
        if plain_value != current_plain_value {
            // For the aforementioned snapping
            let normalized_plain_value = self.param.preview_normalized(plain_value);
            shell.publish(ParamMessage::SetParameterNormalized(
                self.param.as_ptr(),
                normalized_plain_value,
            ));
        }
    }
}

impl<'a, P: Param, Renderer> Widget<ParamMessage, Renderer> for ParamSlider<'a, P, Renderer> 
where
    Renderer: TextRenderer,
    Renderer::Theme: TextInputStyleSheet + TextStyleSheet,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

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

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor_position: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, ParamMessage>,
        viewport: &Rectangle,
    ) -> event::Status {
        // The pressence of a value in `self.state.text_input_value` indicates that the field should
        // be focussed. The field handles defocussing by itself
        // FIMXE: This is super hacky, I have no idea how you can reuse the text input widget
        //        otherwise. Widgets are not supposed to handle messages from other widgets, but
        //        we'll do so anyways by using a special `TextInputMessage` type and our own
        //        `Shell`.
        let state = tree.state.downcast_mut::<State>();
        let text_input_status = if let Some(current_value) = state.text_input_value.clone() {
            let event = event.clone();
            let mut messages = Vec::new();
            let mut text_input_shell = Shell::new(&mut messages);
            let status = event::Status::Ignored;
            // let status = self.with_text_input(
            //     state,
            //     layout,
            //     renderer,
            //     current_value.as_str(),
            //     |mut text_input, layout, renderer| {
            //         text_input.on_event(
            //             tree,
            //             event,
            //             layout,
            //             cursor_position,
            //             renderer,
            //             clipboard,
            //             &mut text_input_shell,
            //             viewport,
            //         )
            //     },
            // );

            // Pressing escape will unfocus the text field, so we should propagate that change in
            // our own model
            if state.text_input_state.borrow().is_focused() {
                for message in messages {
                    match message {
                        TextInputMessage::Value(s) => state.text_input_value = Some(s),
                        TextInputMessage::Submit => {
                            if let Some(normalized_value) = state
                                .text_input_value
                                .as_ref()
                                .and_then(|s| self.param.string_to_normalized_value(s))
                            {
                                shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                                self.set_normalized_value(shell, normalized_value);
                                shell.publish(ParamMessage::EndSetParameter(self.param.as_ptr()));
                            }

                            // And defocus the text input widget again
                            state.text_input_value = None;
                        }
                    }
                }
            } else {
                state.text_input_value = None;
            }

            status
        } else {
            event::Status::Ignored
        };
        if text_input_status == event::Status::Captured {
            return event::Status::Captured;
        }

        // Compensate for the border when handling these events
        let bounds = layout.bounds();
        let bounds = Rectangle {
            x: bounds.x + BORDER_WIDTH,
            y: bounds.y + BORDER_WIDTH,
            width: bounds.width - (BORDER_WIDTH * 2.0),
            height: bounds.height - (BORDER_WIDTH * 2.0),
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let point = cursor_position.position().unwrap_or_default();
                if bounds.contains(point) {
                    let click = mouse::Click::new(point, state.last_click);
                    state.last_click = Some(click);
                    if state.keyboard_modifiers.alt() {
                        // Alt+click should not start a drag, instead it should show the text entry
                        // widget
                        state.drag_active = false;

                        // Changing the parameter happens in the TextInput event handler above
                        let mut text_input_state = state.text_input_state.borrow_mut();
                        state.text_input_value = Some(self.param.to_string());
                        text_input_state.move_cursor_to_end();
                        text_input_state.select_all();
                    } else if state.keyboard_modifiers.command()
                        || matches!(click.kind(), mouse::click::Kind::Double)
                    {
                        // Likewise resetting a parameter should not let you immediately drag it to a new value
                        state.drag_active = false;

                        shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                        self.set_normalized_value(shell, self.param.default_normalized_value());
                        shell.publish(ParamMessage::EndSetParameter(self.param.as_ptr()));
                    } else if state.keyboard_modifiers.shift() {
                        shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                        state.drag_active = true;

                        // When holding down shift while clicking on a parameter we want to
                        // granuarly edit the parameter without jumping to a new value
                        state.granular_drag_start_x_value =
                            Some((point.x, self.param.modulated_normalized_value()));
                    } else {
                        shell.publish(ParamMessage::BeginSetParameter(self.param.as_ptr()));
                        state.drag_active = true;

                        self.set_normalized_value(
                            shell,
                            util::remap_rect_x_coordinate(&bounds, point.x),
                        );
                        state.granular_drag_start_x_value = None;
                    }

                    return event::Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                if state.drag_active {
                    shell.publish(ParamMessage::EndSetParameter(self.param.as_ptr()));

                    state.drag_active = false;

                    return event::Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Touch(touch::Event::FingerMoved { .. }) => {
                let point = cursor_position.position().unwrap_or_default();
                // Don't do anything when we just reset the parameter because that would be weird
                if state.drag_active {
                    // If shift is being held then the drag should be more granular instead of
                    // absolute
                    if state.keyboard_modifiers.shift() {
                        let (drag_start_x, drag_start_value) = *state
                            .granular_drag_start_x_value
                            .get_or_insert_with(|| {
                                (point.x, self.param.modulated_normalized_value())
                            });

                        self.set_normalized_value(
                            shell,
                            util::remap_rect_x_coordinate(
                                &bounds,
                                util::remap_rect_x_t(&bounds, drag_start_value)
                                    + (point.x - drag_start_x) * GRANULAR_DRAG_MULTIPLIER,
                            ),
                        );
                    } else {
                        state.granular_drag_start_x_value = None;

                        self.set_normalized_value(
                            shell,
                            util::remap_rect_x_coordinate(&bounds, point.x),
                        );
                    }

                    return event::Status::Captured;
                }
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.keyboard_modifiers = modifiers;
                let point = cursor_position.position().unwrap_or_default();

                // If this happens while dragging, snap back to reality uh I mean the current screen
                // position
                if state.drag_active
                    && state.granular_drag_start_x_value.is_some()
                    && !modifiers.shift()
                {
                    state.granular_drag_start_x_value = None;

                    self.set_normalized_value(
                        shell,
                        util::remap_rect_x_coordinate(&bounds, point.x),
                    );
                }

                return event::Status::Captured;
            }
            _ => {}
        }

        event::Status::Ignored
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor_position: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let is_mouse_over = bounds.contains(cursor_position.position().unwrap_or_default());

        if is_mouse_over {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Renderer::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor_position: Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        // I'm sure there's some philosophical meaning behind this
        let bounds_without_borders = Rectangle {
            x: bounds.x + BORDER_WIDTH,
            y: bounds.y + BORDER_WIDTH,
            width: bounds.width - (BORDER_WIDTH * 2.0),
            height: bounds.height - (BORDER_WIDTH * 2.0),
        };
        let is_mouse_over = bounds.contains(cursor_position.position().unwrap_or_default());

        // The bar itself, show a different background color when the value is being edited or when
        // the mouse is hovering over it to indicate that it's interactive
        let background_color =
            if is_mouse_over || state.drag_active || state.text_input_value.is_some() {
                Color::new(0.5, 0.5, 0.5, 0.1)
            } else {
                Color::TRANSPARENT
            };

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border_color: Color::BLACK,
                border_width: BORDER_WIDTH,
                border_radius: [0.0; 4].into(),
            },
            background_color,
        );

        // Only draw the text input widget when it gets focussed. Otherwise, overlay the label with
        // the slider.
        if let Some(current_value) = &state.text_input_value {
            self.with_text_input(
                &state,
                layout,
                renderer,
                current_value,
                |text_input, layout, renderer| {
                    text_input.draw(tree, renderer, theme, layout, cursor_position, None)
                },
            )
        } else {
            // We'll visualize the difference between the current value and the default value if the
            // default value lies somewhere in the middle and the parameter is continuous. Otherwise
            // this appraoch looks a bit jarring.
            let current_value = self.param.modulated_normalized_value();
            let default_value = self.param.default_normalized_value();
            let fill_start_x = util::remap_rect_x_t(
                &bounds_without_borders,
                if self.param.step_count().is_none() && (0.45..=0.55).contains(&default_value) {
                    default_value
                } else {
                    0.0
                },
            );
            let fill_end_x = util::remap_rect_x_t(&bounds_without_borders, current_value);

            let fill_color = Color::from_rgb8(196, 196, 196);
            let fill_rect = Rectangle {
                x: fill_start_x.min(fill_end_x),
                width: (fill_end_x - fill_start_x).abs(),
                ..bounds_without_borders
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: fill_rect,
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                    border_radius: [0.0; 4].into(),
                },
                fill_color,
            );

            // To make it more readable (and because it looks cool), the parts that overlap with the
            // fill rect will be rendered in white while the rest will be rendered in black.
            let display_value = self.param.to_string();
            let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
            let font = self.font.unwrap_or_else(|| renderer.default_font());
            let text_bounds = Rectangle {
                x: bounds.center_x(),
                y: bounds.center_y(),
                ..bounds
            };
            renderer.fill_text(text::Text {
                content: &display_value,
                font,
                size: text_size,
                bounds: text_bounds,
                color: style.text_color,
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Center,
                line_height: text::LineHeight::default(),
                shaping: text::Shaping::Basic
            });

            // This will clip to the filled area
            renderer.with_layer(fill_rect, |renderer| {
                let filled_text_color = Color::from_rgb8(80, 80, 80);
                renderer.fill_text(text::Text {
                    content: &display_value,
                    font,
                    size: text_size,
                    bounds: text_bounds,
                    color: filled_text_color,
                    horizontal_alignment: alignment::Horizontal::Center,
                    vertical_alignment: alignment::Vertical::Center,
                    line_height: text::LineHeight::default(),
                    shaping: text::Shaping::Basic
                });
            });
        }
    }
}

impl<'a, P: Param, Renderer> ParamSlider<'a, P, Renderer> 
where 
    Renderer: 'a + TextRenderer,
    Renderer::Theme: TextInputStyleSheet + TextStyleSheet,
{
    /// Convert this [`ParamSlider`] into an [`Element`] with the correct message. You should have a
    /// variant on your own message type that wraps around [`ParamMessage`] so you can forward those
    /// messages to
    /// [`IcedEditor::handle_param_message()`][crate::IcedEditor::handle_param_message()].
    pub fn map<Message, F>(self, f: F) -> Element<'a, Message, Renderer>
    where
        Message: 'static,
        F: Fn(ParamMessage) -> Message + 'static,
    {
        Element::from(self).map(f)
    }
}

impl<'a, P: Param, Renderer> From<ParamSlider<'a, P, Renderer>> for Element<'a, ParamMessage, Renderer> 
where
    Renderer: 'a + TextRenderer,
    Renderer::Theme: TextInputStyleSheet + TextStyleSheet,
{
    fn from(widget: ParamSlider<'a, P, Renderer>) -> Self {
        Element::new(widget)
    }
}