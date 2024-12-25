use crate::gamepad::GamepadManager;
use crate::*;
use core::cell::Cell;
use core::fmt::Display;
use core::marker::PhantomData;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use rodio::Source;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::mpsc;

const UDP_PORT_MIN: u16 = 3110;
const UDP_PORT_MAX: u16 = 3117;
const TCP_PORT_MIN: u16 = 3210;
const TCP_PORT_MAX: u16 = 3217;
const AUDIO_BUF_SIZE: usize = SAMPLE_RATE as usize / 6;

#[derive(Clone)]
pub struct DeviceConfig {
    /// The full path to the VFS.
    pub root: PathBuf,

    /// The TCP IP address where to listen for serial events.
    pub tcp_ip: IpAddr,

    /// The UDP IP address where to listen for netplay events.
    pub udp_ip: IpAddr,

    /// The UDP IP addresses where to send netplay advertisements.
    pub peers: Vec<IpAddr>,

    /// If provided, the path where to save the audio output (as a WAV file).
    pub wav: Option<PathBuf>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        let localhost = IpAddr::V4(Ipv4Addr::LOCALHOST);
        Self {
            root: PathBuf::new(),
            tcp_ip: localhost,
            udp_ip: localhost,
            peers: vec![localhost],
            wav: None,
        }
    }
}

pub struct DeviceImpl<'a> {
    config: DeviceConfig,
    /// The time at which the device instance was created.
    start: std::time::Instant,
    /// The shared logic for reading the gamepad input.
    gamepad: GamepadManager,
    /// The audio buffer
    audio: AudioWriter,
    _life: &'a PhantomData<()>,
}

impl<'a> DeviceImpl<'a> {
    pub fn new(config: DeviceConfig) -> Self {
        let audio = start_audio(&config);
        Self {
            start: std::time::Instant::now(),
            gamepad: GamepadManager::new(),
            audio,
            config,
            _life: &PhantomData,
        }
    }

    /// Called by the GUI to set input from UI and keyboard.
    pub fn update_input(&mut self, input: InputState) {
        self.gamepad.update_input(input)
    }
}

impl<'a> Device for DeviceImpl<'a> {
    type Network = NetworkImpl<'a>;
    type Read = File;
    type Serial = SerialImpl;
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

    fn open_file(&mut self, path: &[&str]) -> Result<Self::Read, FSError> {
        let path: PathBuf = path.iter().collect();
        let path = self.config.root.join(path);
        let file = std::fs::File::open(path)?;
        Ok(File { file })
    }

    fn create_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let path: PathBuf = path.iter().collect();
        let path = self.config.root.join(path);
        if let Some(parent) = path.parent() {
            _ = std::fs::create_dir_all(parent);
        }
        let file = std::fs::File::create(path)?;
        Ok(File { file })
    }

    fn append_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let path: PathBuf = path.iter().collect();
        let path = self.config.root.join(path);
        let mut opts = std::fs::OpenOptions::new();
        let file = opts.append(true).open(path)?;
        Ok(File { file })
    }

    fn get_file_size(&mut self, path: &[&str]) -> Result<u32, FSError> {
        let path: PathBuf = path.iter().collect();
        let path = self.config.root.join(path);
        let meta = std::fs::metadata(path)?;
        Ok(meta.len() as u32)
    }

    fn remove_file(&mut self, path: &[&str]) -> Result<(), FSError> {
        let path: PathBuf = path.iter().collect();
        let path = self.config.root.join(path);
        let res = std::fs::remove_file(path);
        match res {
            Ok(_) => Ok(()),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(err.into()),
            },
        }
    }

    fn iter_dir<F>(&mut self, path: &[&str], mut f: F) -> Result<(), FSError>
    where
        F: FnMut(EntryKind, &[u8]),
    {
        let path: PathBuf = path.iter().collect();
        let path = self.config.root.join(path);
        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
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
        Ok(())
    }

    fn has_headphones(&mut self) -> bool {
        false
    }

    fn get_audio_buffer(&mut self) -> &mut [i16] {
        self.audio.get_write_buf()
    }

    fn network(&mut self) -> Self::Network {
        NetworkImpl::new(self.config.clone())
    }

    fn serial(&self) -> Self::Serial {
        SerialImpl::new(self.config.tcp_ip)
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

pub struct NetworkImpl<'a> {
    config: DeviceConfig,
    worker: Cell<Option<UdpWorker>>,
    r_in: mpsc::Receiver<NetMessage>,
    s_out: mpsc::Sender<NetMessage>,
    s_stop: mpsc::Sender<()>,
    local_addr: Option<SocketAddr>,
    _life: &'a PhantomData<()>,
}

impl<'a> NetworkImpl<'a> {
    fn new(config: DeviceConfig) -> Self {
        let (s_in, r_in) = mpsc::channel();
        let (s_out, r_out) = mpsc::channel();
        let (s_stop, r_stop) = mpsc::channel();
        let worker = Cell::new(Some(UdpWorker {
            s_in,
            r_out,
            r_stop,
        }));
        Self {
            config,
            worker,
            r_in,
            s_out,
            s_stop,
            local_addr: None,
            _life: &PhantomData,
        }
    }
}

pub type Addr = SocketAddr;

impl<'a> Network for NetworkImpl<'a> {
    type Addr = SocketAddr;

    fn local_addr(&self) -> SocketAddr {
        self.local_addr.unwrap()
    }

    fn start(&mut self) -> NetworkResult<()> {
        let worker = self.worker.replace(None);
        let Some(worker) = worker else {
            return Err(NetworkError::AlreadyInitialized);
        };
        let local_addr = worker.start(self.config.udp_ip)?;
        self.local_addr = Some(local_addr);
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        _ = self.s_stop.send(());
        Ok(())
    }

    fn advertise(&mut self) -> NetworkResult<()> {
        let hello = heapless::Vec::from_slice(b"HELLO").unwrap();
        for ip in &self.config.peers {
            for port in UDP_PORT_MIN..=UDP_PORT_MAX {
                let addr = SocketAddr::new(*ip, port);
                let res = self.s_out.send((addr, hello.clone()));
                if res.is_err() {
                    return Err(NetworkError::NetThreadDeallocated);
                }
            }
        }
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>> {
        Ok(self.r_in.try_recv().ok())
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()> {
        let Ok(msg) = heapless::Vec::from_slice(data) else {
            return Err(NetworkError::OutMessageTooBig);
        };
        let res = self.s_out.send((addr, msg));
        if res.is_err() {
            return Err(NetworkError::NetThreadDeallocated);
        }
        Ok(())
    }
}

pub struct SerialImpl {
    ip: IpAddr,
    worker: Cell<Option<TcpWorker>>,
    r_in: mpsc::Receiver<SerialMessage>,
    s_out: mpsc::Sender<SerialMessage>,
    s_stop: mpsc::Sender<()>,
}

impl SerialImpl {
    fn new(ip: IpAddr) -> Self {
        let (s_in, r_in) = mpsc::channel();
        let (s_out, r_out) = mpsc::channel();
        let (s_stop, r_stop) = mpsc::channel();
        let worker = TcpWorker {
            s_in,
            r_out,
            r_stop,
        };
        let worker = Cell::new(Some(worker));
        Self {
            ip,
            worker,
            r_in,
            s_out,
            s_stop,
        }
    }
}

impl Serial for SerialImpl {
    fn start(&mut self) -> NetworkResult<()> {
        let worker = self.worker.replace(None);
        let Some(worker) = worker else {
            return Err(NetworkError::AlreadyInitialized);
        };
        worker.start(self.ip)?;
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        _ = self.s_stop.send(());
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<heapless::Vec<u8, 64>>> {
        Ok(self.r_in.try_recv().ok())
    }

    fn send(&mut self, data: &[u8]) -> NetworkResult<()> {
        let Ok(msg) = heapless::Vec::from_slice(data) else {
            return Err(NetworkError::OutMessageTooBig);
        };
        let res = self.s_out.send(msg);
        if res.is_err() {
            return Err(NetworkError::NetThreadDeallocated);
        }
        Ok(())
    }
}
type NetMessage = (SocketAddr, heapless::Vec<u8, 64>);
type SerialMessage = heapless::Vec<u8, 64>;

struct UdpWorker {
    s_in: mpsc::Sender<NetMessage>,
    r_out: mpsc::Receiver<NetMessage>,
    r_stop: mpsc::Receiver<()>,
}

impl UdpWorker {
    fn start(self, ip: IpAddr) -> Result<SocketAddr, NetworkError> {
        let addrs: Vec<_> = (UDP_PORT_MIN..=UDP_PORT_MAX)
            .map(|port| SocketAddr::new(ip, port))
            .collect();
        let socket = match UdpSocket::bind(&addrs[..]) {
            Ok(socket) => socket,
            Err(_) => return Err(NetworkError::CannotBind),
        };
        let timeout = std::time::Duration::from_millis(10);
        socket.set_read_timeout(Some(timeout)).unwrap();
        if let Ok(addr) = socket.local_addr() {
            println!("listening on {addr}/udp");
        } else {
            println!("listening a UDP port");
        }
        let local_addr = socket.local_addr().unwrap();
        std::thread::spawn(move || loop {
            match self.r_stop.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            let mut buf = vec![0; 64];
            if let Ok((size, addr)) = socket.recv_from(&mut buf) {
                if size == 0 {
                    continue;
                }
                let buf = heapless::Vec::from_slice(&buf[..size]).unwrap();
                _ = self.s_in.send((addr, buf));
            }
            if let Ok((addr, buf)) = self.r_out.try_recv() {
                if addr == local_addr {
                    continue;
                }
                _ = socket.send_to(&buf, addr);
            }
        });
        Ok(local_addr)
    }
}

struct TcpWorker {
    s_in: mpsc::Sender<SerialMessage>,
    r_out: mpsc::Receiver<SerialMessage>,
    r_stop: mpsc::Receiver<()>,
}

impl TcpWorker {
    fn start(self, ip: IpAddr) -> Result<(), NetworkError> {
        let addrs: Vec<_> = (TCP_PORT_MIN..=TCP_PORT_MAX)
            .map(|port| SocketAddr::new(ip, port))
            .collect();
        let socket = match TcpListener::bind(&addrs[..]) {
            Ok(socket) => socket,
            Err(_) => return Err(NetworkError::CannotBind),
        };
        socket.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            let mut streams = RingBuf::new();
            loop {
                match self.r_stop.try_recv() {
                    Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }

                if let Ok((stream, _addr)) = socket.accept() {
                    stream.set_nonblocking(true).unwrap();
                    streams.push(stream);
                };

                for stream in streams.iter_mut() {
                    let mut buf = vec![0; 64];
                    let Ok(size) = stream.read(&mut buf) else {
                        continue;
                    };
                    if size == 0 {
                        continue;
                    }
                    let buf = heapless::Vec::from_slice(&buf[..size]).unwrap();
                    _ = self.s_in.send(buf);
                }
                if let Ok(buf) = self.r_out.try_recv() {
                    for stream in streams.iter_mut() {
                        _ = stream.write_all(&buf)
                    }
                }
            }
        });
        Ok(())
    }
}

/// A collection that holds 4 latest TCP connections.
///
/// If there are already 4 TCP connections and a new one comes in,
/// the oldest one is dropped.
struct RingBuf {
    data: [Option<TcpStream>; 4],
    next: usize,
}

impl RingBuf {
    fn new() -> Self {
        Self {
            data: [None, None, None, None],
            next: 0,
        }
    }

    fn push(&mut self, val: TcpStream) {
        self.data[self.next] = Some(val);
        self.next = (self.next + 1) % 4
    }

    fn iter_mut(
        &mut self,
    ) -> std::iter::FilterMap<
        core::slice::IterMut<Option<TcpStream>>,
        impl FnMut(&mut Option<TcpStream>) -> Option<&mut TcpStream>,
    > {
        self.data.iter_mut().filter_map(Option::as_mut)
    }
}

fn start_audio(config: &DeviceConfig) -> AudioWriter {
    let wav = if let Some(filename) = &config.wav {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(filename, spec).unwrap();
        Some(writer)
    } else {
        None
    };

    let (send, recv) = mpsc::sync_channel(AUDIO_BUF_SIZE);
    let (stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let source = AudioReader { wav, recv };
    stream_handle.play_raw(source.convert_samples()).unwrap();
    AudioWriter {
        buf: [0; AUDIO_BUF_SIZE],
        idx: 0,
        send,
        _stream: stream,
        _stream_handle: stream_handle,
    }
}

struct AudioWriter {
    buf: [i16; AUDIO_BUF_SIZE],
    send: mpsc::SyncSender<i16>,
    /// The index of the next sample that we'll need to try sending.
    idx: usize,

    _stream: rodio::OutputStream,
    _stream_handle: rodio::OutputStreamHandle,
}

impl AudioWriter {
    fn get_write_buf(&mut self) -> &mut [i16] {
        if self.idx == AUDIO_BUF_SIZE {
            self.idx = 0;
        }
        let start = self.idx;
        let mut idx = self.idx;
        // write as much as we can from the buffer into the channel
        while idx < AUDIO_BUF_SIZE {
            let res = self.send.try_send(self.buf[idx]);
            if res.is_err() {
                break;
            }
            idx += 1;
        }
        self.idx = idx;
        // fill the now empty part of the buffer with audio data
        &mut self.buf[start..idx]
    }
}

struct AudioReader {
    wav: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
    recv: mpsc::Receiver<i16>,
}

impl rodio::Source for AudioReader {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<core::time::Duration> {
        None
    }
}

impl Iterator for AudioReader {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        let s = self.recv.try_recv().unwrap_or_default();
        if let Some(wav) = self.wav.as_mut() {
            wav.write_sample(s).unwrap()
        }
        Some(s)
    }
}
