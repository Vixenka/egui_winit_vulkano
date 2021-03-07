mod conversions;

use std::time::Instant;

use crate::conversions::WinitToEgui;
use copypasta::{ClipboardContext, ClipboardProvider};
use egui::{paint::ClippedShape, CtxRef, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use winit::event::{ElementState, MouseScrollDelta};
use winit::{
    dpi::PhysicalSize,
    event::{Event, ModifiersState, VirtualKeyCode, WindowEvent},
};

pub struct EguiContext {
    context: CtxRef,
    raw_input: RawInput,
    mouse_pos: Pos2,
    modifiers_state: ModifiersState,
    clipboard: ClipboardContext,
    start_time: Option<Instant>,
}

impl EguiContext {
    /// Starts a new [`EguiContext`] which is necessary to tie tie egui to the
    /// winit events
    pub fn new(size: PhysicalSize<u32>, scale_factor: f64) -> Self {
        let context = CtxRef::default();
        EguiContext {
            context,
            raw_input: RawInput {
                pixels_per_point: Some(scale_factor as f32),
                screen_rect: Some(Self::screen_rect(size, scale_factor)),
                time: Some(0.0),
                ..Default::default()
            },
            mouse_pos: Pos2::new(0.0, 0.0),
            modifiers_state: ModifiersState::default(),
            clipboard: ClipboardContext::new().expect("Failed to initialize ClipboardContext."),
            start_time: None,
        }
    }

    /// Updates time elapsed since last frame
    /// This should be called every frame
    pub fn update_elapsed_time(&mut self) {
        if let Some(time) = self.start_time {
            self.raw_input.time = Some(time.elapsed().as_secs_f64());
        } else {
            self.start_time = Some(Instant::now());
        }
    }

    /// Begins recording egui frame. Immediate UI should be created between this and
    /// `end_frame`
    pub fn begin_frame(&mut self) {
        self.context.begin_frame(self.raw_input.take());
    }

    /// Ends egui frame recording. Returns a vector of clipped meshes which contain vertices
    /// and indices to be drawn by the integration
    pub fn end_frame(&mut self) -> (egui::Output, Vec<ClippedShape>) {
        let (output, clipped_shapes) = self.context.end_frame();
        // Handles links
        if let Some(url) = &output.open_url {
            if let Err(err) = webbrowser::open(url) {
                eprintln!("Failed to open url: {}", err);
            }
        }
        // Handles clipboard
        if !output.copied_text.is_empty() {
            if let Err(err) = self.clipboard.set_contents(output.copied_text.clone()) {
                eprintln!("Copy/Cut error: {}", err);
            }
        }
        (output, clipped_shapes)
    }

    /// Get [`egui::CtxRef`].
    pub fn context(&self) -> CtxRef {
        self.context.clone()
    }

    /// Update [`EguiContext`] based on winit events
    pub fn handle_event<T>(&mut self, winit_event: &Event<T>) {
        match winit_event {
            Event::WindowEvent {
                window_id: _window_id,
                event,
            } => match event {
                WindowEvent::Resized(physical_size) => {
                    let pixels_per_point = self
                        .raw_input
                        .pixels_per_point
                        .unwrap_or_else(|| self.context.pixels_per_point());
                    self.raw_input.screen_rect =
                        Some(Self::screen_rect(*physical_size, pixels_per_point as f64));
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                } => {
                    self.raw_input.pixels_per_point = Some(*scale_factor as f32);
                    let pixels_per_point = self
                        .raw_input
                        .pixels_per_point
                        .unwrap_or_else(|| self.context.pixels_per_point());
                    self.raw_input.screen_rect =
                        Some(Self::screen_rect(**new_inner_size, pixels_per_point as f64));
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some(button) = WinitToEgui::mouse_button(*button) {
                        self.raw_input.events.push(egui::Event::PointerButton {
                            pos: self.mouse_pos,
                            button,
                            pressed: *state == ElementState::Pressed,
                            modifiers: WinitToEgui::modifiers(self.modifiers_state),
                        });
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        let line_height = 24.0;
                        self.raw_input.scroll_delta = Vec2::new(*x, *y) * line_height;
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        self.raw_input.scroll_delta = Vec2::new(delta.x as f32, delta.y as f32);
                    }
                },
                WindowEvent::CursorMoved { position, .. } => {
                    let pixels_per_point = self
                        .raw_input
                        .pixels_per_point
                        .unwrap_or_else(|| self.context.pixels_per_point());
                    let pos = Pos2::new(
                        position.x as f32 / pixels_per_point,
                        position.y as f32 / pixels_per_point,
                    );
                    self.raw_input.events.push(egui::Event::PointerMoved(pos));
                    self.mouse_pos = pos;
                }
                WindowEvent::CursorLeft { .. } => {
                    self.raw_input.events.push(egui::Event::PointerGone);
                }
                WindowEvent::ModifiersChanged(input) => self.modifiers_state = *input,
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(virtual_keycode) = input.virtual_keycode {
                        let pressed = input.state == ElementState::Pressed;
                        if pressed {
                            let is_ctrl = self.modifiers_state.ctrl();
                            if is_ctrl && virtual_keycode == VirtualKeyCode::C {
                                self.raw_input.events.push(egui::Event::Copy);
                            } else if is_ctrl && virtual_keycode == VirtualKeyCode::X {
                                self.raw_input.events.push(egui::Event::Cut);
                            } else if is_ctrl && virtual_keycode == VirtualKeyCode::V {
                                if let Ok(contents) = self.clipboard.get_contents() {
                                    self.raw_input.events.push(egui::Event::Text(contents));
                                }
                            } else if let Some(key) = WinitToEgui::key_code(virtual_keycode) {
                                self.raw_input.events.push(egui::Event::Key {
                                    key,
                                    pressed: input.state == ElementState::Pressed,
                                    modifiers: WinitToEgui::modifiers(self.modifiers_state),
                                })
                            }
                        }
                    }
                }
                WindowEvent::ReceivedCharacter(ch) => {
                    if ch.is_ascii_control() {
                        return;
                    }
                    self.raw_input
                        .events
                        .push(egui::Event::Text(ch.to_string()));
                }
                _ => (),
            },
            _ => (),
        }
    }

    /// Returns the screen rect based on size & scale factor (pixels per point)
    fn screen_rect(size: PhysicalSize<u32>, scale_factor: f64) -> Rect {
        Rect::from_min_size(
            Default::default(),
            Vec2::new(size.width as f32, size.height as f32) / scale_factor as f32,
        )
    }
}

#[cfg(test)]
mod tests {}
