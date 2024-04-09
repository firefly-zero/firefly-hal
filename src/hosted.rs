use crate::shared::*;
use gilrs::ev::state::AxisData;
use gilrs::*;
use std::path::PathBuf;
use std::time::Duration;

pub struct DeviceImpl {
    start:      std::time::Instant,
    gilrs:      Gilrs,
    gamepad_id: Option<GamepadId>,
    root:       PathBuf,
}

impl Device for DeviceImpl {
    type Read = File;

    fn new(root: &str) -> Self {
        let start = std::time::Instant::now();
        let mut gilrs = Gilrs::new().unwrap();
        let gamepad_id = gilrs.next_event().map(|Event { id, .. }| id);
        Self {
            start,
            gilrs,
            gamepad_id,
            root: PathBuf::new().join(root),
        }
    }

    fn now(&self) -> Time {
        let now = std::time::Instant::now();
        let dur = now.duration_since(self.start);
        fugit::Instant::<u32, 1, 1000>::from_ticks(dur.as_millis() as u32)
    }

    fn delay(&self, d: Delay) {
        let dur = Duration::from_millis(d.to_millis() as u64);
        std::thread::sleep(dur);
    }

    fn read_input(&mut self) -> Option<InputState> {
        if self.gamepad_id.is_none() {
            self.gamepad_id = self.gilrs.next_event().map(|Event { id, .. }| id);
        }
        while self.gilrs.next_event().is_some() {}
        let gamepad_id = self.gamepad_id?;
        let gamepad = self.gilrs.connected_gamepad(gamepad_id)?;
        let left = make_point(
            gamepad.axis_data(Axis::LeftStickX),
            gamepad.axis_data(Axis::LeftStickY),
        );
        let right = make_point(
            gamepad.axis_data(Axis::RightStickX),
            gamepad.axis_data(Axis::RightStickY),
        );
        let menu = gamepad.is_pressed(Button::Start);
        Some(InputState { left, right, menu })
    }

    fn log_debug(&self, src: &str, msg: &str) {
        println!("DEBUG({src}): {msg}");
    }

    fn log_error(&self, src: &str, msg: &str) {
        eprintln!("ERROR({src}): {msg}");
    }

    fn open_file(&self, path: &[&str]) -> Option<Self::Read> {
        let path: PathBuf = path.iter().collect();
        let path = self.root.join(path);
        let file = std::fs::File::open(path).ok()?;
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
}

fn make_point(x: Option<&AxisData>, y: Option<&AxisData>) -> Option<StickPos> {
    let x = data_to_i16(x);
    let y = data_to_i16(y);
    match (x, y) {
        (Some(x), Some(y)) => Some(StickPos { x, y }),
        _ => None,
    }
}

fn data_to_i16(v: Option<&AxisData>) -> Option<i16> {
    let v = v?;
    let v = v.value();
    let r = (v * 1000.) as i16;
    Some(r)
}

pub struct File {
    file: std::fs::File,
}

impl wasmi::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::ReadError> {
        let res = std::io::Read::read(&mut self.file, buf);
        res.map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => wasmi::ReadError::EndOfStream,
            _ => wasmi::ReadError::UnknownError,
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
