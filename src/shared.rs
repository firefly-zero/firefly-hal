use core::fmt;
use core::fmt::Display;
use core::ops::Sub;
use std::ops::AddAssign;

pub enum NetworkError {
    NotInitialized,
    AlreadyInitialized,
    UnknownPeer,
    CannotBind,
    PeerListFull,
    RecvError,
    SendError,
    NetThreadDeallocated,
    OutMessageTooBig,
    Other(u32),
}

impl Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use NetworkError::*;
        match self {
            NotInitialized => write!(f, "cannot send messages with Wi-Fi turned off"),
            AlreadyInitialized => write!(f, "tried to initialize networking twice"),
            UnknownPeer => write!(f, "cannot send messages to disconnected device"),
            CannotBind => write!(f, "cannot find free address for networking"),
            PeerListFull => write!(f, "cannot connect more devices"),
            RecvError => write!(f, "cannot fetch network message"),
            SendError => write!(f, "cannot send network message"),
            NetThreadDeallocated => write!(f, "thread handling networking is already deallocated"),
            OutMessageTooBig => write!(f, "outgoing message is too big"),
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

    pub const fn from_ms(ms: u32) -> Self {
        Self { ns: ms * 1_000_000 }
    }

    pub fn ns(&self) -> u32 {
        self.ns
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

impl AddAssign for Duration {
    fn add_assign(&mut self, rhs: Self) {
        self.ns = self.ns.saturating_add(rhs.ns)
    }
}

pub trait Device {
    type Read: wasmi::Read + embedded_io::Read;
    type Write: embedded_io::Write;
    type Network: Network;
    type Serial: Serial;

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

    /// Access the USB serial port.
    ///
    /// Both read and write operations are non-blocking.
    fn serial(&self) -> Self::Serial;
}

pub(crate) type NetworkResult<T> = Result<T, NetworkError>;

pub trait Network {
    /// The type representing the network address. Must be unique.
    ///
    /// For emulator, it is IP+port. For the physical device, it is MAC address.
    type Addr: Ord;

    fn start(&mut self) -> NetworkResult<()>;
    fn stop(&mut self) -> NetworkResult<()>;

    /// Network address of the current device as visible to the other peers.
    ///
    /// Used to sort all the peers, including the local one, in the same order
    /// on all devices.
    fn local_addr(&self) -> Self::Addr;
    fn advertise(&mut self) -> NetworkResult<()>;

    /// Get a pending message, if any. Non-blocking.
    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>>;

    /// Send a raw message to the given device. Non-blocking.
    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()>;
}

pub trait Serial {
    fn start(&mut self) -> NetworkResult<()>;
    fn stop(&mut self) -> NetworkResult<()>;
    fn recv(&mut self) -> NetworkResult<Option<heapless::Vec<u8, 64>>>;
    fn send(&mut self, data: &[u8]) -> NetworkResult<()>;
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

impl From<(i16, i16)> for Pad {
    fn from(value: (i16, i16)) -> Self {
        Self {
            x: value.0,
            y: value.1,
        }
    }
}

impl From<Pad> for (i16, i16) {
    fn from(value: Pad) -> Self {
        (value.x, value.y)
    }
}

#[derive(Default, Clone, Debug)]
pub struct InputState {
    pub pad: Option<Pad>,
    pub buttons: u8,
}

impl InputState {
    pub fn a(&self) -> bool {
        self.buttons & 0b1 > 0
    }

    pub fn b(&self) -> bool {
        self.buttons & 0b10 > 0
    }

    pub fn x(&self) -> bool {
        self.buttons & 0b100 > 0
    }

    pub fn y(&self) -> bool {
        self.buttons & 0b1000 > 0
    }

    pub fn menu(&self) -> bool {
        self.buttons & 0b10000 > 0
    }

    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            pad: match &self.pad {
                Some(pad) => Some(pad.clone()),
                None => other.pad.clone(),
            },
            buttons: self.buttons | other.buttons,
        }
    }
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
