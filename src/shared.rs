use fugit::{Instant, MillisDurationU32};

pub type Time = Instant<u32, 1, 1000>;
pub type Delay = MillisDurationU32;

pub trait Device {
    type Read: wasmi::Read + embedded_io::Read;

    fn new(root: &'static str) -> Self;

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

    /// Log a debug message into console.
    ///
    /// On hosted environments, it just prints into stdout.
    /// On embedded systems, use [defmt].
    ///
    /// [defmt]: https://defmt.ferrous-systems.com/introduction
    fn log_debug(&self, src: &str, msg: &str);

    /// Log an error into console.
    ///
    /// On hosted environments, it just prints into stderr.
    /// On embedded systems, use [defmt].
    ///
    /// [defmt]: https://defmt.ferrous-systems.com/introduction
    fn log_error(&self, src: &str, msg: &str);

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
    ///
    /// The whole filesystem abstraction (this method and theo nes below)
    /// is designed to work nicely with [embedded_sdmmc] and the stdlib filesystem.
    ///
    /// [embedded_sdmmc]: https://github.com/rust-embedded-community/embedded-sdmmc-rs
    fn open_file(&self, path: &[&str]) -> Option<Self::Read>;

    /// Get file size in bytes.
    ///
    /// None should be returned if file not found.
    fn get_file_size(&self, path: &[&str]) -> Option<u32>;

    /// Create the directory and all its parents if doesn't exist.
    ///
    /// Returns false only if there is an error.
    fn make_dir(&self, path: &[&str]) -> bool;

    /// Delete the given file if exists.
    ///
    /// Directories cannot be removed.
    ///
    /// Returns false only if there is an error.
    fn remove_file(&self, path: &[&str]) -> bool;
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

// (func (param $originalPtr i32)
//   (param $originalSize i32)
//   (param $alignment i32)
//   (param $newSize i32)
//   (result i32))
