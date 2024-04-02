use crate::shared::*;
use gilrs::ev::state::AxisData;
use gilrs::*;
use std::cell::OnceCell;
use std::time::Duration;

static mut DEVICE: OnceCell<HostedDevice> = OnceCell::new();

pub struct HostedDevice {
    start:      std::time::Instant,
    gilrs:      Gilrs,
    gamepad_id: Option<GamepadId>,
}

pub fn get_device() -> &'static mut HostedDevice {
    unsafe {
        DEVICE.get_or_init(|| {
            let start = std::time::Instant::now();
            let mut gilrs = Gilrs::new().unwrap();
            let gamepad_id = gilrs.next_event().map(|Event { id, .. }| id);
            HostedDevice {
                start,
                gilrs,
                gamepad_id,
            }
        });
        DEVICE.get_mut().unwrap()
    }
}

impl Device for HostedDevice {
    fn now(&self) -> Time {
        let now = std::time::Instant::now();
        let dur = now.duration_since(self.start);
        fugit::Instant::<u32, 1, 1000>::from_ticks(dur.as_millis() as u32)
    }

    fn delay(&self, d: Delay) {
        let dur = Duration::from_millis(d.to_millis() as u64);
        std::thread::sleep(dur);
    }

    fn read_input(&mut self) -> Option<InputState> {
        let gamepad_id = self.gamepad_id?;
        let gamepad = self.gilrs.connected_gamepad(gamepad_id)?;
        let left = make_point(
            gamepad.axis_data(Axis::LeftStickX),
            gamepad.axis_data(Axis::LeftStickY),
        );
        let right = make_point(
            gamepad.axis_data(Axis::RightStickX),
            gamepad.axis_data(Axis::RightStickY),
        );
        let menu = gamepad.is_pressed(Button::Start);
        Some(InputState { left, right, menu })
    }
}

fn make_point(x: Option<&AxisData>, y: Option<&AxisData>) -> Option<StickPos> {
    let x = data_to_i16(x);
    let y = data_to_i16(y);
    match (x, y) {
        (Some(x), Some(y)) => Some(StickPos { x, y }),
        _ => None,
    }
}

fn data_to_i16(v: Option<&AxisData>) -> Option<i16> {
    let v = v?;
    let v = v.value();
    let r = (v * 1000.) as i16;
    Some(r)
}
