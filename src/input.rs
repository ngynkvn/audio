use std::path::PathBuf;

pub struct InputHandler {
    pub raw_input: egui::RawInput,
    pub pointer_position: egui::Pos2,
}
impl InputHandler {
    pub fn raw(&mut self) -> egui::RawInput {
        std::mem::take(&mut self.raw_input)
    }
}

pub struct AudioPlayer {
    pub path: Option<PathBuf>,
}
