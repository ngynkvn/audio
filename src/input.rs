use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, Stream,
};
use crossbeam::channel::{unbounded, Receiver};

pub struct InputHandler {
    pub raw_input: egui::RawInput,
    pub pointer_position: egui::Pos2,
}
impl InputHandler {
    pub fn raw(&mut self) -> egui::RawInput {
        std::mem::take(&mut self.raw_input)
    }
}

type AudioBuffer = [f32; 500];
pub struct AudioPlayer {
    pub path: Option<PathBuf>,
    pub rx: Option<Receiver<AudioBuffer>>,
    pub buffer: AudioBuffer,
}

impl AudioPlayer {
    pub fn read_rx(&mut self) {
        if let Some(rx) = &self.rx {
            match rx.recv() {
                Ok(buffer) => self.buffer = buffer,
                Err(e) => {
                    dbg!(e);
                }
            }
        }
    }
    pub fn play(
        &mut self,
        path: PathBuf,
        output_device: &Device,
        output_stream: &mut Option<Stream>,
    ) {
        self.path.replace(path.clone());
        let data = std::fs::read(path.clone()).expect("Could not open file");
        let (header, mut samples) = puremp3::read_mp3(Cursor::new(data)).unwrap();

        let config = output_device.default_output_config().unwrap().config();
        let channels = config.channels;
        let (tx, rx) = unbounded::<AudioBuffer>();
        self.rx.replace(rx);
        let mut buffer: AudioBuffer = [0.0; 500];
        let buffer_len = self.buffer.len();

        output_stream.replace(
            output_device
                .build_output_stream(
                    &config,
                    move |data: &mut [f32], info| {
                        for (frame_index, frame) in data.chunks_mut(channels as _).enumerate() {
                            if let Some((ls, rs)) = samples.next() {
                                frame[0] = ls * 0.3;
                                frame[1] = rs * 0.3;
                            }
                        }
                        buffer.copy_from_slice(&data[..buffer_len]);
                        tx.send(buffer).unwrap();
                    },
                    |err| panic!(),
                )
                .unwrap(),
        );
        output_stream.as_ref().unwrap().play().unwrap();
    }
}
