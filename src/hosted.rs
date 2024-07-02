use crate::gamepad::GamepadManager;
use crate::shared::*;
use core::fmt::Display;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::mpsc;

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
    // worker: std::thread::JoinHandle<()>,
    r_in:  mpsc::Receiver<Message>,
    s_out: mpsc::Sender<Message>,
}

impl NetworkImpl {
    fn new() -> Self {
        let (s_in, r_in) = mpsc::channel();
        let (s_out, r_out) = mpsc::channel();
        let worker = UdpWorker { s_in, r_out };
        _ = worker.start(); // TODO: handle error
        Self { r_in, s_out }
    }
}

impl Network for NetworkImpl {
    type Addr = SocketAddr;

    fn start(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn advertise(&mut self) -> NetworkResult<()> {
        for addr in ADDRESSES {
            let hello = heapless::Vec::from_slice(b"HELLO").unwrap();
            self.s_out.send((*addr, hello)).unwrap();
        }
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>> {
        Ok(self.r_in.try_recv().ok())
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()> {
        let msg = heapless::Vec::from_slice(data).unwrap();
        self.s_out.send((addr, msg)).unwrap();
        Ok(())
    }
}

type Message = (SocketAddr, heapless::Vec<u8, 64>);

struct UdpWorker {
    s_in:  mpsc::Sender<Message>,
    r_out: mpsc::Receiver<Message>,
}

impl UdpWorker {
    fn start(self) -> Result<std::thread::JoinHandle<()>, NetworkError> {
        let socket = match UdpSocket::bind(ADDRESSES) {
            Ok(socket) => socket,
            Err(_) => return Err(NetworkError::Other(0)),
        };
        let timeout = std::time::Duration::from_millis(10);
        socket.set_read_timeout(Some(timeout)).unwrap();
        if let Ok(addr) = socket.local_addr() {
            println!("listening on {addr}");
        }
        let handle = std::thread::spawn(move || loop {
            let mut buf = vec![0; 64];
            if let Ok((size, addr)) = socket.recv_from(&mut buf) {
                if size > 0 {
                    let buf = heapless::Vec::from_slice(&buf[..size]).unwrap();
                    self.s_in.send((addr, buf)).unwrap();
                }
            }
            if let Ok((addr, buf)) = self.r_out.try_recv() {
                if let Ok(local_addr) = socket.local_addr() {
                    if local_addr == addr {
                        continue;
                    }
                }
                socket.send_to(&buf, addr).unwrap();
            }
        });
        Ok(handle)
    }
}
