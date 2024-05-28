use crate::shared::*;
use gilrs::ev::state::AxisData;
use gilrs::*;

/// A gilrs-powered gamepad input reader.
///
/// Shared between the hosted and the web device implementations.
pub(crate) struct GamepadManager {
    gilrs:      Gilrs,
    gamepad_id: Option<GamepadId>,
    input:      InputState,
}

impl GamepadManager {
    pub fn new() -> Self {
        let mut gilrs = Gilrs::new().unwrap();
        let gamepad_id = gilrs.next_event().map(|Event { id, .. }| id);
        Self {
            gilrs,
            gamepad_id,
            input: InputState::default(),
        }
    }

    pub fn update_input(&mut self, input: InputState) {
        self.input = input;
    }

    pub fn read_input(&mut self) -> Option<InputState> {
        // Detect gamepad
        if self.gamepad_id.is_none() {
            self.gamepad_id = self.gilrs.next_event().map(|Event { id, .. }| id);
        }
        // Consume all pending events to update the state
        while self.gilrs.next_event().is_some() {}
        let Some(gamepad_id) = self.gamepad_id else {
            return Some(self.input.clone());
        };
        let gamepad = self.gilrs.connected_gamepad(gamepad_id)?;
        let pad_pressed =
            gamepad.is_pressed(Button::LeftTrigger) | gamepad.is_pressed(Button::LeftThumb);
        let pad = if pad_pressed {
            make_point(
                gamepad.axis_data(Axis::LeftStickX),
                gamepad.axis_data(Axis::LeftStickY),
            )
        } else {
            None
        };
        let buttons = [
            gamepad.is_pressed(Button::South), // A
            gamepad.is_pressed(Button::East),  // B
            gamepad.is_pressed(Button::West),  // X
            gamepad.is_pressed(Button::North), // Y
            gamepad.is_pressed(Button::Start),
        ];

        // merge together input from gamepad and from keyboard
        let buttons = [
            self.input.buttons[0] || buttons[0],
            self.input.buttons[1] || buttons[1],
            self.input.buttons[2] || buttons[2],
            self.input.buttons[3] || buttons[3],
            self.input.buttons[4] || buttons[4],
        ];
        let pad = match pad {
            Some(pad) => Some(pad),
            None => self.input.pad.clone(),
        };

        Some(InputState { pad, buttons })
    }
}

fn make_point(x: Option<&AxisData>, y: Option<&AxisData>) -> Option<Pad> {
    let x = data_to_i16(x);
    let y = data_to_i16(y);
    match (x, y) {
        (Some(x), Some(y)) => Some(Pad { x, y }),
        _ => None,
    }
}

fn data_to_i16(v: Option<&AxisData>) -> Option<i16> {
    let v = v?;
    let v = v.value();
    let r = (v * 1000.) as i16;
    Some(r)
}
