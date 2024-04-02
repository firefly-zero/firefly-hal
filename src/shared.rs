use fugit::{Instant, MillisDurationU32};

pub type Time = Instant<u32, 1, 1000>;
pub type Delay = MillisDurationU32;

pub trait Device<'a> {
    type Read: wasmi::Read + embedded_io::Read;

    fn new(root: &'a str) -> Self;

    /// The current time.
    ///
    /// Should be precise enough for adjusting the delay between frames.
    ///
    /// Usually implemented as [rtic_time.Monotonic].
    /// May also sometimes be implemented as [rtic_monotonic.Monotonic].
    ///
    /// [rtic_time.Monotonic]: https://docs.rs/rtic-time/latest/rtic_time/trait.Monotonic.html
    /// [rtic_monotonic.Monotonic]: https://docs.rs/rtic-monotonic/latest/rtic_monotonic/trait.Monotonic.html
    fn now(&self) -> Time;

    /// Suspends the current thread for the given duration.
    ///
    /// Should be precise enough for adjusting the delay between frames.
    ///
    /// Usually implemented as [embedded_hal.DelayNs].
    ///
    /// [embedded_hal.DelayNs]: https://docs.rs/embedded-hal/1.0.0/embedded_hal/delay/trait.DelayNs.html
    fn delay(&self, d: Delay);

    /// Read gamepad input.
    fn read_input(&mut self) -> Option<InputState>;

    /// Open a file for reading.
    ///
    /// The file path is given as a slice of path components.
    /// There are at least 4 components:
    ///
    /// 1. the first one is the root directory (either "roms" or "data"),
    /// 2. the second is the author ID,
    /// 3. the third is the app ID,
    /// 4. (optional) directory names if the file is nested,
    /// 5. and the last is file name.
    ///
    /// The runtime ensures that the path is relative and never goes up the tree.
    fn open_file(&self, path: &[&str]) -> Option<Self::Read>;
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
