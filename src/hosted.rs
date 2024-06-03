use crate::gamepad::GamepadManager;
use crate::shared::*;
use core::fmt::Display;
use std::path::PathBuf;

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
