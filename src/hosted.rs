use crate::shared::*;
use gilrs::ev::state::AxisData;
use gilrs::*;
use std::cell::OnceCell;
use std::time::Duration;

static mut START: OnceCell<std::time::Instant> = OnceCell::new();
static mut GILRS: OnceCell<Gilrs> = OnceCell::new();
static mut GAMEPAD_ID: Option<GamepadId> = None;

fn get_start_time() -> std::time::Instant {
    *unsafe { START.get_or_init(std::time::Instant::now) }
}

fn get_gilrs() -> &'static mut Gilrs {
    unsafe {
        GILRS.get_or_init(|| Gilrs::new().unwrap());
        GILRS.get_mut().unwrap()
    }
}

fn get_gamepad_id() -> Option<GamepadId> {
    match unsafe { GAMEPAD_ID } {
        Some(gamepad) => Some(gamepad),
        None => {
            let gilrs = get_gilrs();
            if let Some(Event { id, .. }) = gilrs.next_event() {
                let gamepad = Some(id);
                unsafe { GAMEPAD_ID = gamepad }
                return gamepad;
            }
            None
        }
    }
}

pub fn now() -> fugit::Instant<u32, 1, 1000> {
    let now = std::time::Instant::now();
    let start = get_start_time();
    let dur = now.duration_since(start);
    fugit::Instant::<u32, 1, 1000>::from_ticks(dur.as_millis() as u32)
}

pub fn delay(d: fugit::MillisDurationU32) {
    let dur = Duration::from_millis(d.to_millis() as u64);
    std::thread::sleep(dur);
}

pub fn read_input() -> Option<InputState> {
    let gilrs = get_gilrs();
    let gamepad_id = get_gamepad_id()?;
    let gamepad = gilrs.connected_gamepad(gamepad_id)?;
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
