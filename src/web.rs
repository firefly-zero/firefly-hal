use core::fmt::Display;
use core::marker::PhantomData;
use std::collections::HashMap;

use rust_embed::RustEmbed;
use vfs::FileSystem;
use wasm_bindgen::prelude::*;

use crate::gamepad::GamepadManager;
use crate::{FSError, shared::*};

#[derive(RustEmbed, Debug)]
#[folder = "$VFS"]
struct Vfs;

static mut RO_VFS: Option<vfs::EmbeddedFS<Vfs>> = None;

static mut RW_VFS: Option<HashMap<String, Vec<u8>>> = None;

#[expect(static_mut_refs)]
fn get_vfs() -> (
    &'static mut vfs::EmbeddedFS<Vfs>,
    &'static mut HashMap<String, Vec<u8>>,
) {
    unsafe { (RO_VFS.as_mut().unwrap(), RW_VFS.as_mut().unwrap()) }
}

#[wasm_bindgen]
extern {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(msg: &str);

    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(msg: &str);

    #[wasm_bindgen(js_namespace = Math, js_name = random)]
    fn math_random() -> f64;
}

#[derive(Default)]
pub struct DeviceConfig {}

pub struct DeviceImpl<'a> {
    start: f64,
    gamepad: GamepadManager,
    perf: web_sys::Performance,
    wifi_status: firefly_types::wifi::Status,
    nothing: PhantomData<&'a str>,
}

impl<'a> DeviceImpl<'a> {
    pub fn new(_: DeviceConfig) -> Self {
        unsafe {
            RO_VFS = Some(vfs::EmbeddedFS::new());
            RW_VFS = Some(HashMap::new());
        }
        let window = web_sys::window().unwrap();
        let perf = window.performance().unwrap();
        Self {
            gamepad: GamepadManager::new(),
            start: perf.now(),
            perf,
            wifi_status: firefly_types::wifi::Status::Stopped,
            nothing: PhantomData,
        }
    }

    /// Called by the GUI to set input from UI and keyboard.
    pub fn update_input(&mut self, input: InputState) {
        self.gamepad.update_input(input)
    }

    pub fn alloc_psram(&self, size: usize) -> Vec<u8> {
        Vec::with_capacity(size)
    }
}

impl<'a> Device for DeviceImpl<'a> {
    type Dir = DirImpl;

    fn now(&self) -> Instant {
        let ms = self.perf.now() - self.start;
        let us = ms as u32 * 1000;
        Instant { us }
    }

    fn delay(&self, _d: Duration) {
        // TODO: find a way to block the thread.
    }

    fn read_input(&mut self) -> Option<InputState> {
        // TODO: read keyboard input as well
        self.gamepad.read_input()
    }

    fn log_debug<D: Display>(&mut self, src: &str, msg: D) {
        console_log(&format!("{src}: {msg}"))
    }

    fn log_error<D: Display>(&mut self, src: &str, msg: D) {
        console_error(&format!("{src}: {msg}"))
    }

    fn get_name(&mut self) -> Option<&'static str> {
        None
    }

    fn random(&mut self) -> u32 {
        (math_random() * u32::MAX as f64) as u32
    }

    fn open_dir(&mut self, path: &[&str]) -> Result<Self::Dir, FSError> {
        let path = path.join("/");
        let path = format!("/{path}");
        Ok(DirImpl { root: path })
    }

    fn has_headphones(&mut self) -> bool {
        false
    }

    fn get_battery_status(&mut self) -> Option<BatteryStatus> {
        None
    }

    fn write_main_flash(&mut self, _offset: u32, _data: &[u8]) -> Result<(), &'static str> {
        Ok(())
    }

    fn write_io_flash(&mut self, _offset: u32, _data: &[u8]) -> NetworkResult<()> {
        Ok(())
    }

    fn switch_main_partition(&mut self, _part: u8) -> Result<(), &'static str> {
        Ok(())
    }

    fn switch_io_partition(&mut self, _part: u8) -> NetworkResult<()> {
        Ok(())
    }
}

impl<'a> Wifi for DeviceImpl<'a> {
    fn wifi_scan(&mut self) -> NetworkResult<[String; 6]> {
        let points = ["Default Network", "", "", "", "", ""];
        let points = points.map(|s| s.to_string());
        Ok(points)
    }

    fn wifi_connect(&mut self, ssid: &str, pass: &str) -> NetworkResult<()> {
        use firefly_types::wifi::{DisconnectReason, Status};
        self.wifi_status = if ssid != "Default Network" {
            Status::Disconnected(DisconnectReason::NoApFound)
        } else if pass == "invalid" {
            Status::Disconnected(DisconnectReason::AuthFail)
        } else {
            Status::Initializing
        };
        Ok(())
    }

    fn wifi_status(&mut self) -> NetworkResult<u8> {
        use firefly_types::wifi::Status;
        if self.wifi_status == Status::Initializing {
            self.wifi_status = Status::Connected;
        }
        Ok(self.wifi_status.into())
    }

    fn wifi_disconnect(&mut self) -> NetworkResult<()> {
        self.wifi_status = firefly_types::wifi::Status::Stopped;
        Ok(())
    }

    fn tcp_connect(&mut self, _ip: u32, _port: u16) -> NetworkResult<()> {
        Ok(())
    }

    fn tcp_status(&mut self) -> NetworkResult<u8> {
        Ok(0)
    }

    fn tcp_send(&mut self, _data: &[u8]) -> NetworkResult<()> {
        Ok(())
    }

    fn tcp_recv(&mut self) -> NetworkResult<Box<[u8]>> {
        Ok(Box::new([]))
    }

    fn tcp_close(&mut self) -> NetworkResult<()> {
        Ok(())
    }
}

pub type Addr = ();

impl<'a> Network for DeviceImpl<'a> {
    type Addr = ();

    fn net_start(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn net_stop(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn net_local_addr(&self) -> Self::Addr {}

    fn net_advertise(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn net_recv(&mut self) -> NetworkResult<Option<(Self::Addr, Box<[u8]>)>> {
        Ok(None)
    }

    fn net_send(&mut self, _addr: Self::Addr, _dataa: &[u8]) -> NetworkResult<()> {
        Ok(())
    }

    fn net_send_status(
        &mut self,
        _addr: Self::Addr,
    ) -> NetworkResult<firefly_types::spi::SendStatus> {
        Ok(firefly_types::spi::SendStatus::Delivered(1))
    }
}

impl<'a> Serial for DeviceImpl<'a> {
    fn serial_start(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn serial_stop(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn serial_recv(&mut self) -> NetworkResult<Option<Box<[u8]>>> {
        Ok(None)
    }

    fn serial_send(&mut self, _data: &[u8]) -> NetworkResult<()> {
        Ok(())
    }
}

pub struct DirImpl {
    root: String,
}

impl Dir for DirImpl {
    type Read = FileR;
    type Write = FileW;

    fn open_file(&mut self, name: &str) -> Result<Self::Read, FSError> {
        let path = format!("{}/{name}", self.root);
        let (ro_vfs, rw_vfs) = get_vfs();
        if rw_vfs.keys().any(|k| k.starts_with(&path)) {
            return Ok(FileR::Path(path, 0));
        }
        let file = wrap(ro_vfs.open_file(&path))?;
        Ok(FileR::File(file))
    }

    fn create_file(&mut self, name: &str) -> Result<Self::Write, FSError> {
        let path = format!("{}/{name}", self.root);
        let (_, rw_vfs) = get_vfs();
        rw_vfs.insert(path.clone(), Vec::new());
        Ok(FileW { path })
    }

    fn append_file(&mut self, name: &str) -> Result<Self::Write, FSError> {
        let path = format!("{}/{name}", self.root);
        let (ro_vfs, rw_vfs) = get_vfs();

        if ro_vfs.exists(&path).unwrap_or_default() {
            let mut buf = Vec::new();
            let mut file = wrap(ro_vfs.open_file(&path))?;
            _ = file.read_to_end(&mut buf);
            rw_vfs.insert(path.clone(), buf);
            return Ok(FileW { path });
        }

        if !rw_vfs.keys().any(|k| k.starts_with(&path)) {
            return Err(FSError::NotFound);
        }
        Ok(FileW { path })
    }

    fn get_file_size(&mut self, name: &str) -> Result<u32, FSError> {
        let path = format!("{}/{name}", self.root);
        let (ro_vfs, rw_vfs) = get_vfs();
        if let Some(f) = rw_vfs.get(&path) {
            return Ok(f.len() as u32);
        }
        let meta = wrap(ro_vfs.metadata(&path))?;
        Ok(meta.len as u32)
    }

    fn remove_file(&mut self, name: &str) -> Result<(), FSError> {
        let path = format!("{}/{name}", self.root);
        let (ro_vfs, rw_vfs) = get_vfs();
        if ro_vfs.exists(&path).unwrap_or_default() {
            let err = "file is readonly and cannot be removed";
            return Err(FSError::FormatError(err));
        }
        _ = rw_vfs.remove(&path);
        Ok(())
    }

    fn create_dir(&mut self, _name: &str) -> Result<(), FSError> {
        Ok(())
    }

    fn remove_dir(self) -> Result<(), FSError> {
        Ok(())
    }

    fn iter_dir<F>(&mut self, mut f: F) -> Result<(), FSError>
    where
        F: FnMut(EntryKind, &[u8]),
    {
        // TODO: read from rw_vfs as well.
        let (ro_vfs, _rw_vfs) = get_vfs();
        let entries = wrap(ro_vfs.read_dir(&self.root))?;
        for fname in entries {
            let path = format!("{}/{fname}", self.root);
            let meta = wrap(ro_vfs.metadata(&path))?;
            let kind = if meta.file_type == vfs::VfsFileType::Directory {
                EntryKind::Dir
            } else if meta.file_type == vfs::VfsFileType::File {
                EntryKind::File
            } else {
                continue;
            };
            let fname = fname.as_bytes();
            f(kind, fname);
        }
        Ok(())
    }
}

pub enum FileR {
    File(Box<dyn vfs::SeekAndRead + Send>),
    Path(String, usize),
}

impl embedded_io::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            FileR::File(file) => std::io::Read::read(file, buf),
            FileR::Path(path, pos) => {
                let (_, rw_vfs) = get_vfs();
                let file = rw_vfs.get_mut(path).unwrap();
                let file = &file[*pos..];
                let to_read = file.len().min(buf.len());
                buf[..to_read].copy_from_slice(&file[..to_read]);
                *pos += to_read;
                Ok(to_read)
            }
        }
    }
}

impl embedded_io::Seek for FileR {
    fn seek(&mut self, pos: embedded_io::SeekFrom) -> Result<u64, Self::Error> {
        match self {
            FileR::File(file) => file.seek(pos.into()),
            FileR::Path(path, pos_tar) => {
                match pos {
                    embedded_io::SeekFrom::Start(new_pos) => {
                        *pos_tar = new_pos as usize;
                    }
                    embedded_io::SeekFrom::End(new_pos) => {
                        let (_, rw_vfs) = get_vfs();
                        let file = rw_vfs.get_mut(path).unwrap();
                        *pos_tar = file.len() - new_pos as usize;
                    }
                    embedded_io::SeekFrom::Current(new_pos) => {
                        *pos_tar += new_pos as usize;
                    }
                }
                Ok(*pos_tar as u64)
            }
        }
    }
}

impl embedded_io::ErrorType for FileR {
    type Error = std::io::Error;
}

pub struct FileW {
    path: String,
}

impl embedded_io::Write for FileW {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let (_, rw_vfs) = get_vfs();
        let file = rw_vfs.get_mut(&self.path).unwrap();
        file.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_io::ErrorType for FileW {
    type Error = std::io::Error;
}

fn wrap<T>(r: Result<T, vfs::VfsError>) -> Result<T, FSError> {
    use vfs::error::VfsErrorKind;
    match r {
        Ok(v) => Ok(v),
        Err(err) => match err.kind() {
            VfsErrorKind::FileNotFound => Err(FSError::NotFound),
            _ => Err(FSError::DeviceError(format!("{err}"))),
        },
    }
}
