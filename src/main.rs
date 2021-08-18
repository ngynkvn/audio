extern crate glium;

mod draw;
mod input;
use std::{io::Cursor, iter, path::PathBuf};

use color_eyre::{owo_colors::Color, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, Stream, StreamConfig, SupportedInputConfigs,
};
use crossbeam::channel::{Receiver, Sender};
use draw::draw_frame;
use egui::plot::Value;
use egui::plot::Values;
use egui::Stroke;
use egui::{plot::Line, Pos2};
use egui::{plot::Plot, Vec2};
use egui::{plot::Points, Modifiers};
use glium::{
    draw_parameters,
    glutin::{
        self,
        dpi::PhysicalPosition,
        event::{self, DeviceEvent, ModifiersState, MouseScrollDelta, WindowEvent},
        event::{ElementState, Event},
        event_loop::{ControlFlow, EventLoop},
    },
    implement_vertex,
    index::{NoIndices, PrimitiveType},
    uniform, Display, DrawParameters, IndexBuffer, Program, Surface, VertexBuffer,
};

const VERT: &str = "
#version 330 core
in vec2 vert;
void main() {
    gl_Position = vec4(vert, 0.0, 1.0);
}";

const FRAG: &str = "
#version 330 core

out vec4 color;

void main()
{    
    color = vec4(1.0, 0.0, 0.0, 1.0);
}";

#[derive(Clone, Copy)]
pub struct Vertex {
    pub vert: [f32; 2],
}

implement_vertex!(Vertex, vert);

impl Vertex {
    fn new<I: Into<f32>>(x: I, y: I) -> Self {
        Self {
            vert: [x.into(), y.into()],
        }
    }
}

use puremp3::Mp3Decoder;
use rustfft::{num_complex::Complex32, FftPlanner};

use crate::input::{AudioPlayer, InputHandler};

type Buffer = [Complex32; 480];

struct AudioInfo {
    host: Host,
    input_device: Device,
    input_stream: Stream,
    output_device: Device,
    output_stream: Option<Stream>,
}

fn init_audio(output: Sender<[f32; 480]>) -> Result<AudioInfo> {
    puffin::profile_function!();
    // let mut planner = FftPlanner::<f32>::new();
    // let fft = planner.plan_fft_forward(480);
    // let mut buffer = [Complex32::default(); 480];
    let mut buffer = [0.0; 480];

    let host = cpal::default_host();
    println!("-- Input Devices --");
    for id in host.input_devices()? {
        println!("{:?}", id.name());
    }
    let input_device = host
        .default_input_device()
        .expect("no input device available");
    let config = input_device.default_input_config()?;
    println!("InputStreamConfigs: {:?}", config);
    let input_stream = input_device.build_input_stream(
        &config.into(),
        move |data: &[f32], info| {
            for (i, c) in data.iter().zip(buffer.iter_mut()) {
                *c = *i;
            }
            // fft.process(&mut buffer);
            output.send(buffer).unwrap();
        },
        |err| panic!(),
    )?;
    let output_device = host
        .default_output_device()
        .expect("no output device available");
    Ok(AudioInfo {
        host,
        input_device,
        input_stream,
        output_device,
        output_stream: None,
    })
}

fn init_graphics() -> Result<(Display, Program, VertexBuffer<Vertex>, EventLoop<()>)> {
    puffin::profile_function!();
    let events_loop = glutin::event_loop::EventLoop::new();
    let cb = glutin::ContextBuilder::new();

    let wb = glutin::window::WindowBuilder::new()
        .with_inner_size(glutin::dpi::LogicalSize::new(1024.0, 768.0))
        .with_title("Hello world");

    let display = glium::Display::new(wb, cb, &events_loop)?;
    let program = glium::Program::from_source(&display, VERT, FRAG, None)?;
    let mut data = [Vertex::new(0.0, 0.0); 256];
    let xlin = ndarray::Array::linspace(-1.0, 1.0, 256);
    for (vertex, x) in data.iter_mut().zip(xlin) {
        vertex.vert[0] = x;
        vertex.vert[1] = 0.2;
    }
    let vertices = VertexBuffer::dynamic(&display, &data)?;
    Ok((display, program, vertices, events_loop))
}

fn main() -> Result<()> {
    color_eyre::install()?;
    std::env::set_var("RUST_BACKTRACE", "1");
    puffin::profile_function!();
    puffin::set_scopes_on(true);
    let server = puffin_http::Server::new("127.0.0.1:8585").unwrap();
    let mut buffer = [Default::default(); 480];
    let (tx, rx) = crossbeam::channel::unbounded();

    let AudioInfo {
        input_device,
        input_stream,
        host,
        output_device,
        mut output_stream,
    } = init_audio(tx)?;
    let (display, _program, _vertices, event_loop) = init_graphics()?;
    let mut egui = egui::CtxRef::default();
    let mut painter = egui_glium::Painter::new(&display);

    let mut audio_player = AudioPlayer { path: None };
    let mut input_handler = InputHandler {
        raw_input: Default::default(),
        pointer_position: Default::default(),
    };
    // let indices = IndexBuffer::new(&display, PrimitiveType::Points, &[0, 1, 2])?;

    input_stream.play()?;
    event_loop.run(move |e, _t, c| {
        puffin::profile_scope!("Event Handler");
        if let Ok(b) = rx.try_recv() {
            buffer = b;
        }

        match e {
            Event::NewEvents(_) => {}
            Event::WindowEvent { window_id, event } => match event {
                WindowEvent::CloseRequested => {
                    *c = ControlFlow::Exit;
                }
                WindowEvent::KeyboardInput {
                    device_id,
                    input,
                    is_synthetic,
                } => {
                    println!("{:?}", input);
                }
                WindowEvent::DroppedFile(path) => {
                    audio_player.path.replace(path.clone());
                    let data = std::fs::read(path.clone()).expect("Could not open file");
                    let (header, samples) = puremp3::read_mp3(Cursor::new(data)).unwrap();
                    let mut samples =
                        samples.flat_map(|(l, r)| Iterator::chain(iter::once(l), iter::once(r)));

                    let config = output_device.default_output_config().unwrap().config();

                    output_stream.replace(
                        output_device
                            .build_output_stream(
                                &config,
                                move |data: &mut [f32], info| {
                                    for d in data {
                                        if let Some(dd) = samples.next() {
                                            *d = dd;
                                        }
                                    }
                                },
                                |err| panic!(),
                            )
                            .unwrap(),
                    );
                    output_stream.as_ref().unwrap().play().unwrap();
                }
                WindowEvent::CursorMoved {
                    position: PhysicalPosition { x, y },
                    ..
                } => {
                    input_handler.pointer_position = Pos2::new(x as _, y as _);
                    input_handler
                        .raw_input
                        .events
                        .push(egui::Event::PointerMoved(input_handler.pointer_position));
                }
                WindowEvent::MouseInput {
                    button: _button,
                    state,
                    ..
                } => {
                    input_handler
                        .raw_input
                        .events
                        .push(egui::Event::PointerButton {
                            pos: input_handler.pointer_position,
                            button: egui::PointerButton::Primary,
                            pressed: state == ElementState::Pressed,
                            modifiers: Default::default(), /* fields */
                        });
                }
                WindowEvent::MouseWheel {
                    delta: MouseScrollDelta::LineDelta(x, y),
                    ..
                } => input_handler.raw_input.scroll_delta = Vec2::new(x, y),
                WindowEvent::ModifiersChanged(mods) => {
                    println!("{:?}", mods);
                    input_handler.raw_input.modifiers = Modifiers {
                        alt: mods.alt(),
                        ctrl: mods.ctrl(),
                        shift: mods.shift(),
                        mac_cmd: false,
                        command: mods.ctrl(),
                    };
                }
                WindowEvent::Resized(_) => {}
                WindowEvent::Moved(_)
                | WindowEvent::Destroyed
                | WindowEvent::HoveredFile(_)
                | WindowEvent::HoveredFileCancelled
                | WindowEvent::ReceivedCharacter(_)
                | WindowEvent::Focused(_)
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::TouchpadPressure { .. }
                | WindowEvent::AxisMotion { .. }
                | WindowEvent::Touch(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::ThemeChanged(_)
                | _ => {}
            },
            Event::MainEventsCleared => {
                puffin::profile_scope!("Plot");
                draw_frame(
                    &display,
                    &mut egui,
                    &buffer,
                    &mut input_handler,
                    &mut painter,
                    &audio_player,
                );
            }
            Event::DeviceEvent {
                event: device_event,
                ..
            } => match device_event {
                DeviceEvent::Added => {}
                DeviceEvent::Removed => {}
                DeviceEvent::MouseMotion { delta } => {}
                DeviceEvent::MouseWheel { delta } => {}
                DeviceEvent::Motion { axis, value } => {}
                DeviceEvent::Button { button, state } => {}
                DeviceEvent::Key(_) => {}
                DeviceEvent::Text { codepoint } => {}
            },
            Event::UserEvent(_)
            | Event::Suspended
            | Event::Resumed
            | Event::RedrawEventsCleared
            | Event::LoopDestroyed => {}
            Event::RedrawRequested(_) => {}
        }
        puffin::GlobalProfiler::lock().new_frame();
        server.update();
    });
}
