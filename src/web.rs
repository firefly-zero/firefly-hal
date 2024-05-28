use crate::gamepad::GamepadManager;
use crate::shared::*;
use core::fmt::Display;
use rust_embed::RustEmbed;
use vfs::FileSystem;
use wasm_bindgen::prelude::*;

#[derive(RustEmbed, Debug)]
#[folder = "/home/gram/.local/share/firefly"]
struct Vfs;

#[wasm_bindgen]
extern {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(msg: &str);

    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(msg: &str);
}

pub struct DeviceImpl {
    // start:      std::time::Instant,
    gamepad: GamepadManager,
    vfs:     vfs::impls::embedded::EmbeddedFS<Vfs>,
    window:  web_sys::Window,
    perf:    web_sys::Performance,
}

impl DeviceImpl {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let window = web_sys::window().unwrap();
        Self {
            gamepad: GamepadManager::new(),
            vfs: vfs::EmbeddedFS::new(),
            perf: window.performance().unwrap(),
            window,
        }
    }
}

impl Device for DeviceImpl {
    type Read = FileR;
    type Write = FileW;

    fn now(&self) -> Time {
        Time::from_ticks(self.perf.now() as u32)
    }

    fn delay(&self, d: Delay) {
        // TODO: find a way to block the thread.
    }

    fn read_input(&mut self) -> Option<InputState> {
        // TODO: read keyboard input as well
        self.gamepad.read_input()
    }

    fn log_debug<D: Display>(&self, src: &str, msg: D) {
        console_log(&format!("{src}: {msg}"))
    }

    fn log_error<D: Display>(&self, src: &str, msg: D) {
        console_error(&format!("{src}: {msg}"))
    }

    fn open_file(&self, path: &[&str]) -> Option<Self::Read> {
        let path = path.join("/");
        let file = self.vfs.open_file(&path).ok()?;
        Some(FileR { file })
    }

    fn create_file(&self, path: &[&str]) -> Option<Self::Write> {
        let path = path.join("/");
        let file = self.vfs.create_file(&path).ok()?;
        Some(FileW { file })
    }

    fn get_file_size(&self, path: &[&str]) -> Option<u32> {
        let path = path.join("/");
        let meta = self.vfs.metadata(&path).ok()?;
        Some(meta.len as u32)
    }

    fn make_dir(&self, path: &[&str]) -> bool {
        let path = path.join("/");
        self.vfs.create_dir(&path).is_ok()
    }

    fn remove_file(&self, path: &[&str]) -> bool {
        let path = path.join("/");
        self.vfs.remove_file(&path).is_ok()
    }

    fn iter_dir<F>(&self, path: &[&str], f: F) -> bool
    where
        F: FnMut(EntryKind, &[u8]),
    {
        // TODO: implement
        false
    }
}

pub struct FileR {
    file: Box<dyn vfs::SeekAndRead + Send>,
}

impl wasmi::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        let res = std::io::Read::read(&mut self.file, buf);
        res.map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => wasmi::errors::ReadError::EndOfStream,
            _ => wasmi::errors::ReadError::UnknownError,
        })
    }
}

impl embedded_io::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        std::io::Read::read(&mut self.file, buf)
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
