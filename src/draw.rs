use std::path::PathBuf;

use egui::{
    plot::{Line, Plot, Points, Value, Values},
    CtxRef, Stroke, Ui,
};
use egui_glium::Painter;
use glium::{Display, Surface};

use crate::input::{AudioPlayer, InputHandler};
const scalar: f32 = 10.0;

fn draw_line_graph(buffer: &[f32]) -> impl FnOnce(&mut Ui) {
    let points = buffer
        .iter()
        .enumerate()
        .map(|(i, c)| Value::new(i as f64, *c as f64));
    let line = Line::new(Values::from_values_iter(points.clone()))
        .stroke(Stroke::new(2.0, egui::Color32::RED));
    let bounds = Points::new(Values::from_values(vec![
        Value::new(0.0, -1.1),
        Value::new(0.0, 1.1),
    ]));
    let plot = Plot::new("Audio").line(line).points(bounds);
    move |ui| {
        ui.label("Line");
        ui.add(plot);
    }
}

fn draw_circle_graph(buffer: &[f32]) -> impl FnOnce(&mut Ui) {
    let linspace =
        ndarray::Array::linspace(-std::f32::consts::PI, std::f32::consts::PI, buffer.len());
    let points = buffer
        .iter()
        .zip(linspace)
        .map(|(y, x)| Value::new((0.2 + y * scalar) * x.cos(), (0.2 + y * scalar) * x.sin()));
    let bounds = Points::new(Values::from_values(vec![
        Value::new(0.0, -1.1),
        Value::new(0.0, 1.1),
        Value::new(-1.0, -1.1),
        Value::new(1.0, 1.1),
    ]));
    let line = Points::new(Values::from_values_iter(points));
    let plot = Plot::new("Audio").points(line).points(bounds);
    move |ui| {
        ui.add(plot);
    }
}

fn draw_audio_player<'a>(
    path: &'a PathBuf,
    audio_player: &'a mut AudioPlayer,
) -> impl FnOnce(&mut Ui) + 'a {
    move |ui| {
        ui.label(format!("Now playing: {:?}", path));
        audio_player.read_rx();
        draw_circle_graph(&audio_player.buffer)(ui);
    }
}

pub fn draw_frame(
    display: &Display,
    egui: &mut CtxRef,
    buffer: &[f32],
    input_handler: &mut InputHandler,
    painter: &mut Painter,
    audio_player: &mut AudioPlayer,
) {
    egui.begin_frame(input_handler.raw());
    egui::Window::new("Line Graph")
        .default_size((300.0, 300.0))
        .show(&egui, draw_line_graph(buffer));

    egui::Window::new("Circle Graph")
        .default_size((300.0, 300.0))
        .show(&egui, draw_circle_graph(buffer));

    if let Some(path) = audio_player.path.clone() {
        egui::Window::new("Now playing")
            .default_size((200.0, 200.0))
            .show(&egui, draw_audio_player(&path, audio_player));
    }

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
