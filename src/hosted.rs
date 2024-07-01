use crate::gamepad::GamepadManager;
use crate::shared::*;
use core::fmt::Display;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::net::UdpSocket;
use std::path::PathBuf;

static LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
static ADDRESSES: &[SocketAddr] = &[
    SocketAddr::new(LOCALHOST, 3110),
    SocketAddr::new(LOCALHOST, 3111),
    SocketAddr::new(LOCALHOST, 3112),
    SocketAddr::new(LOCALHOST, 3113),
    SocketAddr::new(LOCALHOST, 3114),
    SocketAddr::new(LOCALHOST, 3115),
    SocketAddr::new(LOCALHOST, 3116),
    SocketAddr::new(LOCALHOST, 3117),
];

pub struct DeviceImpl {
    start:   std::time::Instant,
    gamepad: GamepadManager,
    root:    PathBuf,
}

impl DeviceImpl {
    pub fn new(root: PathBuf) -> Self {
        Self {
            start: std::time::Instant::now(),
            gamepad: GamepadManager::new(),
            root,
        }
    }

    /// Called by the GUI to set input from UI and keyboard.
    pub fn update_input(&mut self, input: InputState) {
        self.gamepad.update_input(input)
    }
}

impl Device for DeviceImpl {
    type Network = NetworkImpl;
    type Read = File;
    type Write = File;

    fn now(&self) -> Instant {
        let now = std::time::Instant::now();
        let dur = now.duration_since(self.start);
        Instant {
            ns: dur.as_nanos() as u32,
        }
    }

    fn delay(&self, d: Duration) {
        let dur = core::time::Duration::from_nanos(d.ns as u64);
        std::thread::sleep(dur);
    }

    fn read_input(&mut self) -> Option<InputState> {
        self.gamepad.read_input()
    }

    fn log_debug<D: Display>(&self, src: &str, msg: D) {
        println!("DEBUG({src}): {msg}");
    }

    fn log_error<D: Display>(&self, src: &str, msg: D) {
        eprintln!("ERROR({src}): {msg}");
    }

    fn open_file(&self, path: &[&str]) -> Option<Self::Read> {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        let file = std::fs::File::open(path).ok()?;
        Some(File { file })
    }

    fn create_file(&self, path: &[&str]) -> Option<Self::Write> {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        if let Some(parent) = path.parent() {
            _ = std::fs::create_dir_all(parent);
        }
        let file = std::fs::File::create(path).ok()?;
        Some(File { file })
    }

    fn get_file_size(&self, path: &[&str]) -> Option<u32> {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        let Ok(meta) = std::fs::metadata(path) else {
            return None;
        };
        Some(meta.len() as u32)
    }

    fn make_dir(&self, path: &[&str]) -> bool {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        std::fs::create_dir_all(path).is_ok()
    }

    fn remove_file(&self, path: &[&str]) -> bool {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        let res = std::fs::remove_file(path);
        match res {
            Ok(_) => true,
            Err(err) => matches!(err.kind(), std::io::ErrorKind::NotFound),
        }
    }

    fn iter_dir<F>(&self, path: &[&str], mut f: F) -> bool
    where
        F: FnMut(EntryKind, &[u8]),
    {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        let Ok(entries) = std::fs::read_dir(path) else {
            return false;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                return false;
            };
            let path = entry.path();
            let kind = if path.is_dir() {
                EntryKind::Dir
            } else if path.is_file() {
                EntryKind::File
            } else {
                continue;
            };
            let fname = entry.file_name();
            let fname = fname.as_encoded_bytes();
            f(kind, fname);
        }
        true
    }

    fn network(&self) -> Self::Network {
        NetworkImpl::new()
    }
}

pub struct File {
    file: std::fs::File,
}

impl wasmi::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        let res = std::io::Read::read(&mut self.file, buf);
        res.map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => wasmi::errors::ReadError::EndOfStream,
            _ => wasmi::errors::ReadError::UnknownError,
        })
    }
}

impl embedded_io::ErrorType for File {
    type Error = std::io::Error;
}

impl embedded_io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        std::io::Read::read(&mut self.file, buf)
    }
}

impl embedded_io::Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        std::io::Write::write(&mut self.file, buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        std::io::Write::flush(&mut self.file)
    }
}

pub struct NetworkImpl {
    socket: Option<UdpSocket>,
}

impl NetworkImpl {
    fn new() -> Self {
        Self { socket: None }
    }
}

impl Network for NetworkImpl {
    type Addr = SocketAddr;

    fn start(&mut self) -> NetworkResult<()> {
        if self.socket.is_some() {
            return Err(NetworkError::AlreadyInitialized);
        }
        let socket = match UdpSocket::bind(ADDRESSES) {
            Ok(socket) => socket,
            Err(_) => return Err(NetworkError::Other(0)),
        };
        self.socket = Some(socket);
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        if self.socket.is_none() {
            return Err(NetworkError::NotInitialized);
        }
        self.socket = None;
        Ok(())
    }

    fn advertise(&mut self) -> NetworkResult<()> {
        let Some(socket) = &self.socket else {
            return Err(NetworkError::NotInitialized);
        };
        let local_addr = socket.local_addr().ok();
        // TODO: use broadcast or multicast
        for addr in ADDRESSES {
            if let Some(local_addr) = local_addr {
                if addr == &local_addr {
                    continue;
                }
            }
            _ = socket.send_to(b"HELLO", addr);
        }
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>> {
        let Some(socket) = &self.socket else {
            return Err(NetworkError::NotInitialized);
        };
        let mut buf: heapless::Vec<u8, 64> = heapless::Vec::new();
        let Ok((_, addr)) = socket.recv_from(&mut buf) else {
            return Ok(None);
        };
        Ok(Some((addr, buf)))
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()> {
        let Some(socket) = &self.socket else {
            return Err(NetworkError::NotInitialized);
        };
        let res = socket.send_to(data, addr);
        if res.is_err() {
            return Err(NetworkError::Other(0));
        }
        Ok(())
    }
}
