use crate::shared::*;
#[cfg(not(target_os = "android"))]
use gilrs::ev::state::AxisData;
#[cfg(not(target_os = "android"))]
use gilrs::*;

/// A gilrs-powered gamepad input reader.
///
/// Shared between the hosted and the web device implementations.
pub(crate) struct GamepadManager {
    #[cfg(not(target_os = "android"))]
    gilrs: Gilrs,
    #[cfg(not(target_os = "android"))]
    gamepad_id: Option<GamepadId>,
    input: InputState,
}

impl GamepadManager {
    pub fn new() -> Self {
        #[cfg(not(target_os = "android"))]
        let mut gilrs = Gilrs::new().unwrap();
        #[cfg(not(target_os = "android"))]
        let gamepad_id = gilrs.next_event().map(|Event { id, .. }| id);
        Self {
            #[cfg(not(target_os = "android"))]
            gilrs,
            #[cfg(not(target_os = "android"))]
            gamepad_id,
            input: InputState::default(),
        }
    }

    pub fn update_input(&mut self, input: InputState) {
        self.input = input;
    }

    pub fn read_input(&mut self) -> Option<InputState> {
        #[allow(unused_mut)]
        let mut buttons = 0u8;
        #[allow(unused_mut, unused_assignments)]
        let mut pad = None;

        #[cfg(not(target_os = "android"))]
        {
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
            pad = read_pad(gamepad);
            let buttons_array = [
                gamepad.is_pressed(Button::South), // A
                gamepad.is_pressed(Button::East),  // B
                gamepad.is_pressed(Button::West),  // X
                gamepad.is_pressed(Button::North), // Y
                gamepad.is_pressed(Button::Start),
            ];

            for b in buttons_array.into_iter().rev() {
                buttons = buttons << 1 | u8::from(b);
            }
        }

        // merge together input from gamepad and from keyboard
        let buttons = self.input.buttons | buttons;
        let pad = match pad {
            Some(pad) => Some(pad),
            None => self.input.pad.clone(),
        };

        Some(InputState { pad, buttons })
    }
}

#[cfg(not(target_os = "android"))]
/// Read state of sticks and convert it into touchpad state.
fn read_pad(gamepad: Gamepad<'_>) -> Option<Pad> {
    if gamepad.is_pressed(Button::DPadDown) {
        return Some(Pad { x: 0, y: -1000 });
    }
    if gamepad.is_pressed(Button::DPadUp) {
        return Some(Pad { x: 0, y: 1000 });
    }
    if gamepad.is_pressed(Button::DPadLeft) {
        return Some(Pad { x: -1000, y: 0 });
    }
    if gamepad.is_pressed(Button::DPadRight) {
        return Some(Pad { x: 1000, y: 0 });
    }

    // Left stick works as pad only if it is pressed down.
    //
    // The only exceptions is Firefly Zero because
    // it uses touchpad instead of stick and hence has no drift.
    // Steam Controller, despite having two touchpads, by default uses
    // stick for the left coordinate instead of the left touchpad.
    //
    // We don't check vendor ID for Firefly Zero because it's not settled yet.
    // The dev version of the gamepad mode uses Flipper Devices Inc VID (0x37C1).
    let is_precise = gamepad.product_id() == Some(0x1337);
    let pad_pressed =
        gamepad.is_pressed(Button::LeftTrigger) | gamepad.is_pressed(Button::LeftThumb);
    if is_precise || pad_pressed {
        let maybe_point = make_point(
            gamepad.axis_data(Axis::LeftStickX),
            gamepad.axis_data(Axis::LeftStickY),
        );
        let point = maybe_point?;
        if is_precise && point.x == 0 && point.y == 0 {
            return None;
        }
        return Some(point);
    };

    let pad = make_point(
        gamepad.axis_data(Axis::RightStickX),
        gamepad.axis_data(Axis::RightStickY),
    );
    // if right stick is pressed, treat it as pad
    if gamepad.is_pressed(Button::RightThumb) {
        return pad;
    }
    let Pad { x, y } = pad?;
    let x_zero = (-50..=50).contains(&x);
    let y_zero = (-50..=50).contains(&y);
    // if right stick is resting, pad is not pressed
    if x_zero && y_zero {
        return None;
    }
    Some(Pad { x, y })
}

#[cfg(not(target_os = "android"))]
fn make_point(x: Option<&AxisData>, y: Option<&AxisData>) -> Option<Pad> {
    let x = data_to_i16(x);
    let y = data_to_i16(y);
    match (x, y) {
        (Some(x), Some(y)) => Some(Pad { x, y }),
        _ => None,
    }
}

#[cfg(not(target_os = "android"))]
fn data_to_i16(v: Option<&AxisData>) -> Option<i16> {
    let v = v?;
    let v = v.value();
    let r = (v * 1000.) as i16;
    Some(r)
}
