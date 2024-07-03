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
    vfs: vfs::impls::embedded::EmbeddedFS<Vfs>,
    perf: web_sys::Performance,
}

impl DeviceImpl {
    #[allow(clippy::new_without_default)]
    pub fn new(_: std::path::PathBuf) -> Self {
        let window = web_sys::window().unwrap();
        Self {
            gamepad: GamepadManager::new(),
            vfs: vfs::EmbeddedFS::new(),
            perf: window.performance().unwrap(),
        }
    }
}

impl Device for DeviceImpl {
    type Read = FileR;
    type Write = FileW;

    fn now(&self) -> Instant {
        Instant {
            ns: self.perf.now() as u32 * 1_000_000,
        }
    }

    fn delay(&self, _d: Duration) {
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
        let file = self.vfs.open_file(&format!("/{path}")).ok()?;
        Some(FileR { file })
    }

    fn create_file(&self, path: &[&str]) -> Option<Self::Write> {
        let path = path.join("/");
        let file = self.vfs.create_file(&format!("/{path}")).ok()?;
        Some(FileW { file })
    }

    fn get_file_size(&self, path: &[&str]) -> Option<u32> {
        let path = path.join("/");
        let meta = self.vfs.metadata(&format!("/{path}")).ok()?;
        Some(meta.len as u32)
    }

    fn make_dir(&self, path: &[&str]) -> bool {
        let path = path.join("/");
        self.vfs.create_dir(&format!("/{path}")).is_ok()
    }

    fn remove_file(&self, path: &[&str]) -> bool {
        let path = path.join("/");
        self.vfs.remove_file(&format!("/{path}")).is_ok()
    }

    fn iter_dir<F>(&self, path: &[&str], mut f: F) -> bool
    where
        F: FnMut(EntryKind, &[u8]),
    {
        let root = path.join("/");
        let Ok(entries) = self.vfs.read_dir(&format!("/{root}")) else {
            return false;
        };
        for path in entries {
            let path = format!("/{root}/{path}");
            let meta = self.vfs.metadata(&path).unwrap();
            let kind = match meta.file_type {
                vfs::VfsFileType::File => EntryKind::File,
                vfs::VfsFileType::Directory => EntryKind::Dir,
            };
            let fname = path.split('/').last().unwrap();
            f(kind, fname.as_bytes());
        }
        true
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
