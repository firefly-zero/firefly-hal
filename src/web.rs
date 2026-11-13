use core::fmt::Display;

use rust_embed::RustEmbed;
use vfs::FileSystem;
use wasm_bindgen::prelude::*;

use crate::gamepad::GamepadManager;
use crate::{FSError, shared::*};

#[derive(RustEmbed, Debug)]
#[folder = "/home/gram/.local/share/firefly"]
struct Vfs;

static mut VFS: Option<vfs::EmbeddedFS<Vfs>> = None;

#[expect(static_mut_refs)]
fn get_vfs() -> &'static mut vfs::EmbeddedFS<Vfs> {
    unsafe { VFS.as_mut().unwrap() }
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

pub struct DeviceImpl {
    start: f64,
    gamepad: GamepadManager,
    perf: web_sys::Performance,
    wifi_status: firefly_types::wifi::Status,
}

impl DeviceImpl {
    pub fn new(_: std::path::PathBuf) -> Self {
        unsafe { VFS = Some(vfs::EmbeddedFS::new()) };
        let window = web_sys::window().unwrap();
        let perf = window.performance().unwrap();
        Self {
            gamepad: GamepadManager::new(),
            start: perf.now(),
            perf,
            wifi_status: firefly_types::wifi::Status::Stopped,
        }
    }
}

impl Device for DeviceImpl {
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
        math_random() as u32
    }

    fn open_dir(&mut self, path: &[&str]) -> Result<Self::Dir, FSError> {
        let path = path.join("/");
        let path = format!("/{path}");
        let vfs = get_vfs();
        if !vfs.exists(&path).unwrap_or_default() {
            return Err(FSError::NotFound);
        }
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

impl Wifi for DeviceImpl {
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

impl Network for DeviceImpl {
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

impl Serial for DeviceImpl {
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
        let vfs = get_vfs();
        let res = vfs.open_file(&path);
        let Ok(file) = res else {
            return Err(FSError::Other);
        };
        Ok(FileR { file })
    }

    fn create_file(&mut self, name: &str) -> Result<Self::Write, FSError> {
        let path = format!("{}/{name}", self.root);
        let vfs = get_vfs();
        let res = vfs.create_file(&path);
        let Ok(file) = res else {
            return Err(FSError::Other);
        };
        Ok(FileW { file })
    }

    fn append_file(&mut self, name: &str) -> Result<Self::Write, FSError> {
        let path = format!("{}/{name}", self.root);
        let vfs = get_vfs();
        let res = vfs.append_file(&path);
        let Ok(file) = res else {
            return Err(FSError::Other);
        };
        Ok(FileW { file })
    }

    fn get_file_size(&mut self, name: &str) -> Result<u32, FSError> {
        let path = format!("{}/{name}", self.root);
        let vfs = get_vfs();
        let res = vfs.metadata(&path);
        let Ok(meta) = res else {
            return Err(FSError::Other);
        };
        Ok(meta.len as u32)
    }

    fn remove_file(&mut self, name: &str) -> Result<(), FSError> {
        let path = format!("{}/{name}", self.root);
        let vfs = get_vfs();
        let res = vfs.remove_file(&path);
        if res.is_err() {
            return Err(FSError::Other);
        }
        Ok(())
    }

    fn create_dir(&mut self, name: &str) -> Result<(), FSError> {
        let path = format!("{}/{name}", self.root);
        let vfs = get_vfs();
        let res = vfs.create_dir(&path);
        if res.is_err() {
            return Err(FSError::Other);
        }
        Ok(())
    }

    fn remove_dir(self) -> Result<(), FSError> {
        let vfs = get_vfs();
        let res = vfs.remove_dir(&self.root);
        if res.is_err() {
            return Err(FSError::Other);
        }
        Ok(())
    }

    fn iter_dir<F>(&mut self, mut f: F) -> Result<(), FSError>
    where
        F: FnMut(EntryKind, &[u8]),
    {
        let entries = std::fs::read_dir(&self.root)?;
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
}

pub struct FileR {
    file: Box<dyn vfs::SeekAndRead + Send>,
}

impl embedded_io::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        std::io::Read::read(&mut self.file, buf)
    }
}

impl embedded_io::Seek for FileR {
    fn seek(&mut self, pos: embedded_io::SeekFrom) -> Result<u64, Self::Error> {
        self.file.seek(pos.into())
    }
}

impl embedded_io::ErrorType for FileR {
    type Error = std::io::Error;
}

pub struct FileW {
    file: Box<dyn vfs::SeekAndWrite + Send>,
}

impl embedded_io::Write for FileW {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        std::io::Write::write(&mut self.file, buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        std::io::Write::flush(&mut self.file)
    }
}

impl embedded_io::ErrorType for FileW {
    type Error = std::io::Error;
}
