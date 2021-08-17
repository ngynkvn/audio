extern crate glium;

use egui::Stroke;
use color_eyre::{owo_colors::Color, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream,
};
use crossbeam::channel::{Receiver, Sender};
use egui::plot::Line;
use egui::plot::Plot;
use egui::plot::Points;
use egui::plot::Value;
use egui::plot::Values;
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

type Buffer = [Complex32; 480];

fn init_audio(output: Sender<[f32; 480]>) -> Result<(Device, Stream)> {
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
    let device = host
        .default_input_device()
        .expect("no input device available");
    let config = device.default_input_config()?;
    println!("{:?}", config);
    let stream = device.build_input_stream(
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
    Ok((device, stream))
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

    let (input_device, stream) = init_audio(tx)?;
    let (display, _program, _vertices, event_loop) = init_graphics()?;
    let mut egui = egui::CtxRef::default();
    let mut painter = egui_glium::Painter::new(&display);
    // let indices = IndexBuffer::new(&display, PrimitiveType::Points, &[0, 1, 2])?;

    stream.play()?;
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
            Event::MainEventsCleared => {
                puffin::profile_scope!("Plot");
                let points = buffer
                    .iter()
                    .enumerate()
                    .map(|(i, c)| Value::new(i as f64, *c as f64));
                let bounds = Points::new(Values::from_values(vec![Value::new(0.0, -1.1), Value::new(0.0, 1.1)]));
                let line = Line::new(Values::from_values_iter(points)).stroke(Stroke::new(2.0, egui::Color32::RED));
                let plot = Plot::new("Audio").line(line).points(bounds);

                egui.begin_frame(Default::default());
                egui::Window::new("My Window").fixed_size((500.0, 500.0)).show(&egui, |ui| {
                    ui.label("Hi!");
                    ui.add(plot);
                });
                let (_output, shapes) = egui.end_frame();
                let clipped_mesh = egui.tessellate(shapes);
                let mut target = display.draw();
                let scale = display.gl_window().window().scale_factor();
                target.clear_color(0.3, 0.3, 0.3, 1.0);
                painter.paint_meshes(
                    &display,
                    &mut target,
                    scale as f32,
                    clipped_mesh,
                    &egui.texture(),
                );
                target.finish().unwrap();
            }
            Event::DeviceEvent { .. }
            | Event::UserEvent(_)
            | Event::Suspended
            | Event::Resumed
            | Event::RedrawEventsCleared
            | Event::LoopDestroyed | _ => {}
        }
        puffin::GlobalProfiler::lock().new_frame();
        server.update();
    });
}
