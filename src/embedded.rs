pub struct DeviceImpl {
    start: std::time::Instant,
    gilrs: Gilrs,
    gamepad_id: Option<GamepadId>,
}

pub fn get_device() -> &'static mut DeviceImpl {
    todo!()
}

impl Device for DeviceImpl {
    fn now(&self) -> Time {
        todo!()
    }

    fn delay(&self, d: Delay) {
        todo!()
    }

    fn read_input(&mut self) -> Option<InputState> {
        todo!()
    }
}
