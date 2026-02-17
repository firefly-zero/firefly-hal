use crate::shared::*;

pub(crate) struct GamepadManager {
    input: InputState,
}

impl GamepadManager {
    pub fn new() -> Self {
        Self {
            input: InputState::default(),
        }
    }

    pub fn update_input(&mut self, input: InputState) {
        self.input = input;
    }

    pub fn read_input(&mut self) -> Option<InputState> {
        let buttons = self.input.buttons;
        let pad = self.input.pad.clone();
        Some(InputState { pad, buttons })
    }
}
