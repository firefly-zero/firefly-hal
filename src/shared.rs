use fugit::{Instant, MillisDurationU32};

pub type Time = Instant<u32, 1, 1000>;
pub type Delay = MillisDurationU32;

pub trait Device {
    fn now(&self) -> Time;
    fn delay(&self, d: Delay);
    fn read_input(&mut self) -> Option<InputState>;
}

pub struct StickPos {
    pub x: i16,
    pub y: i16,
}

#[derive(Default)]
pub struct InputState {
    pub left:  Option<StickPos>,
    pub right: Option<StickPos>,
    pub menu:  bool,
}
