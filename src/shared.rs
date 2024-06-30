use core::fmt;
use core::fmt::Display;
use core::ops::Sub;

pub enum NetworkError {
    NotInitialized,
    UnknownPeer,
    Other(u32),
}

impl Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use NetworkError::*;
        match self {
            NotInitialized => write!(f, "cannot send messages with Wi-Fi turned off"),
            UnknownPeer => write!(f, "cannot send messages to disconnected device"),
            Other(n) => write!(f, "network error #{n}"),
        }
    }
}

/// A moment in time. Obtained from [Device::now].
#[derive(Copy, Clone)]
pub struct Instant {
    pub(crate) ns: u32,
}

impl Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Duration {
        Duration {
            ns: self.ns.saturating_sub(rhs.ns),
        }
    }
}

/// Difference between two [Instant]'s. Used by [Device::delay].
#[derive(PartialEq, PartialOrd, Copy, Clone)]
pub struct Duration {
    pub(crate) ns: u32,
}

impl Duration {
    /// Given the desired frames per second, get the duration of a single frame.
    pub fn from_fps(fps: u32) -> Self {
        Self {
            ns: 1_000_000_000 / fps,
        }
    }
}

impl Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            ns: self.ns - rhs.ns,
        }
    }
}

pub trait Device {
    type Read: wasmi::Read + embedded_io::Read;
    type Write: embedded_io::Write;
    type Network: Network;

    /// The current time.
    ///
    /// Should be precise enough for adjusting the delay between frames.
    ///
    /// Usually implemented as [rtic_time.Monotonic].
    /// May also sometimes be implemented as [rtic_monotonic.Monotonic].
    ///
    /// [rtic_time.Monotonic]: https://docs.rs/rtic-time/latest/rtic_time/trait.Monotonic.html
    /// [rtic_monotonic.Monotonic]: https://docs.rs/rtic-monotonic/latest/rtic_monotonic/trait.Monotonic.html
    fn now(&self) -> Instant;

    /// Suspends the current thread for the given duration.
    ///
    /// Should be precise enough for adjusting the delay between frames.
    ///
    /// Usually implemented as [embedded_hal.DelayNs].
    ///
    /// [embedded_hal.DelayNs]: https://docs.rs/embedded-hal/1.0.0/embedded_hal/delay/trait.DelayNs.html
    fn delay(&self, d: Duration);

    /// Read gamepad input.
    fn read_input(&mut self) -> Option<InputState>;

    /// Log a debug message into console.
    ///
    /// On hosted environments, it just prints into stdout.
    /// On embedded systems, use [defmt].
    ///
    /// [defmt]: https://defmt.ferrous-systems.com/introduction
    fn log_debug<D: Display>(&self, src: &str, msg: D);

    /// Log an error into console.
    ///
    /// On hosted environments, it just prints into stderr.
    /// On embedded systems, use [defmt].
    ///
    /// [defmt]: https://defmt.ferrous-systems.com/introduction
    fn log_error<D: Display>(&self, src: &str, msg: D);

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

    /// Create a new file and open it for write.
    ///
    /// If the file already exists, it will be overwritten.
    fn create_file(&self, path: &[&str]) -> Option<Self::Write>;

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

    /// Call the callback for each entry in the given directory.
    ///
    /// A better API would be to return an iterator
    /// but embedded-sdmmc-rs [doesn't support it][1].
    ///
    /// [1]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/125
    fn iter_dir<F>(&self, path: &[&str], f: F) -> bool
    where
        F: FnMut(EntryKind, &[u8]);

    fn network(&self) -> Self::Network;
}

type NetworkResult<T> = Result<T, NetworkError>;

pub trait Network {
    type Addr;

    fn start(&mut self);
    fn stop(&mut self);
    fn advertise(&mut self) -> NetworkResult<()>;

    /// Get the list of connected devices.
    fn peers(&mut self) -> &[Self::Addr];

    /// Get a pending message, if any. Non-blocking.
    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>>;

    /// Send a raw message to the given device. Blocking.
    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()>;
}

pub enum EntryKind {
    Dir,
    File,
}

#[derive(Default, Clone, Debug)]
pub struct Pad {
    pub x: i16,
    pub y: i16,
}

#[derive(Default, Clone, Debug)]
pub struct InputState {
    pub pad:     Option<Pad>,
    pub buttons: [bool; 5],
}

// (func (param $originalPtr i32)
//   (param $originalSize i32)
//   (param $alignment i32)
//   (param $newSize i32)
//   (result i32))

// sample rate
// channels

// volume
// speed
// play/pause
// stop
// play_next
