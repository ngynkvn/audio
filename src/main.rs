extern crate glium;

use color_eyre::{owo_colors::Color, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream,
};
use glium::{
    draw_parameters,
    glutin::{
        self,
        event::Event,
        event::{self, WindowEvent},
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

use rustfft::{num_complex::Complex32, FftPlanner};

fn init_audio() -> Result<(Device, Stream, [Complex32; 480])> {
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(480);

    let host = cpal::default_host();
    let mut buffer = [Complex32::new(0.0, 0.0); 480];
    println!("-- Input Devices --");
    for id in host.input_devices()? {
        println!("{:?}", id.name());
    }
    let device = host
        .default_input_device()
        .expect("no input device available");
    let config = device.default_input_config()?;
    println!("{:?}", config);
    let stream = device.build_input_stream(
        &config.into(),
        |data: &[f32], info| {
            for (i, c) in data.iter().zip(buffer.iter_mut()) {
                c.re = *i;
            }
            fft.process(&mut buffer);
        },
        |err| panic!(),
    )?;
    Ok((device, stream, buffer))
}

fn init_graphics() -> Result<(Display, Program, VertexBuffer<Vertex>, EventLoop<()>)> {
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
    std::env::set_var("RUST_BACKTRACE", "1");
    color_eyre::install()?;
    let (input_device, stream, buffer) = init_audio()?;
    let (display, program, vertices, event_loop) = init_graphics()?;
    // let indices = IndexBuffer::new(&display, PrimitiveType::Points, &[0, 1, 2])?;

    stream.play()?;
    event_loop.run(move |e, _t, c| {
        // println!("{:?}", buffer);
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
                WindowEvent::Resized(_) => {}
                WindowEvent::Moved(_)
                | WindowEvent::Destroyed
                | WindowEvent::DroppedFile(_)
                | WindowEvent::HoveredFile(_)
                | WindowEvent::HoveredFileCancelled
                | WindowEvent::ReceivedCharacter(_)
                | WindowEvent::Focused(_)
                | WindowEvent::ModifiersChanged(_)
                | WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::TouchpadPressure { .. }
                | WindowEvent::AxisMotion { .. }
                | WindowEvent::Touch(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::ThemeChanged(_) => {}
            },
            Event::RedrawRequested(_window) => {
                let mut target = display.draw();
                let uniforms = uniform! {};
                target
                    .draw(
                        &vertices,
                        &NoIndices(PrimitiveType::Points),
                        &program,
                        &uniforms,
                        &DrawParameters {
                            point_size: Some(2.0),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                target.finish().unwrap();
            }
            Event::DeviceEvent { .. }
            | Event::UserEvent(_)
            | Event::Suspended
            | Event::Resumed
            | Event::MainEventsCleared
            | Event::RedrawEventsCleared
            | Event::LoopDestroyed => {}
        }
    });
}
