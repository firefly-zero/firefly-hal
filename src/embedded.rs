use crate::shared::*;
use core::cell::Cell;
use embedded_storage::{ReadStorage, Storage};
use esp_hal::{clock::CpuClock, delay::Delay, uart::Uart, Blocking};
use esp_storage::FlashStorage;
use fugit::MicrosDurationU64;

pub struct DeviceImpl {
    delay: Delay,
    uart: Cell<Option<Uart<'static, Blocking>>>,
    flash: FlashStorage,
}

impl DeviceImpl {
    pub fn new(uart: Uart<'static, Blocking>) -> Result<Self, esp_hal::uart::Error> {
        let device = Self {
            delay: Delay::new(),
            uart: Cell::new(Some(uart)),
            flash: FlashStorage::new(),
        };
        Ok(device)
    }

    fn log(&self, msg: &str) {
        let mut uart = self.uart.replace(None);
        _ = uart.as_mut().unwrap().write_bytes(msg.as_bytes());
        self.uart.replace(uart);
    }
}

impl Device for DeviceImpl {
    type Read = FileR;
    type Write = FileW;
    type Network = NetworkImpl;
    type Serial = SerialImpl;

    fn now(&self) -> Instant {
        todo!()
    }

    fn delay(&self, d: Duration) {
        let d_micros = d.ns() / 1_000;
        let d = MicrosDurationU64::from_ticks(d_micros as u64);
        self.delay.delay(d);
    }

    fn read_input(&mut self) -> Option<InputState> {
        None
    }

    fn log_debug<D: core::fmt::Display>(&self, src: &str, msg: D) {
        let msg = alloc::format!("DEBUG({src}): {msg}");
        self.log(&msg);
    }

    fn log_error<D: core::fmt::Display>(&self, src: &str, msg: D) {
        let msg = alloc::format!("ERROR({src}): {msg}");
        self.log(&msg);
    }

    fn open_file(&self, path: &[&str]) -> Option<Self::Read> {
        // self.flash.read(offset, bytes);
        None
    }

    fn create_file(&self, path: &[&str]) -> Option<Self::Write> {
        None
    }

    fn append_file(&self, path: &[&str]) -> Option<Self::Write> {
        None
    }

    fn get_file_size(&self, path: &[&str]) -> Option<u32> {
        None
    }

    fn make_dir(&self, path: &[&str]) -> bool {
        false
    }

    fn remove_file(&self, path: &[&str]) -> bool {
        false
    }

    fn iter_dir<F>(&self, path: &[&str], f: F) -> bool
    where
        F: FnMut(crate::EntryKind, &[u8]),
    {
        false
    }

    fn network(&self) -> Self::Network {
        NetworkImpl {}
    }

    fn serial(&self) -> Self::Serial {
        SerialImpl {}
    }

    fn has_headphones(&mut self) -> bool {
        false
    }

    fn get_audio_buffer(&mut self) -> &mut [i16] {
        &mut []
    }
}

pub struct FileW {}

impl embedded_io::ErrorType for FileW {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Write for FileW {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Err(embedded_io::ErrorKind::Other)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Err(embedded_io::ErrorKind::Other)
    }
}

pub struct FileR {}

impl embedded_io::ErrorType for FileR {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err(embedded_io::ErrorKind::Other)
    }
}

impl wasmi::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        Err(wasmi::errors::ReadError::UnknownError)
    }
}

pub struct NetworkImpl {}

impl Network for NetworkImpl {
    type Addr = ();

    fn start(&mut self) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }

    fn stop(&mut self) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }

    fn local_addr(&self) -> Self::Addr {
        ()
    }

    fn advertise(&mut self) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }

    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>> {
        Err(NetworkError::Other(0))
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }
}

pub struct SerialImpl {}

impl Serial for SerialImpl {
    fn start(&mut self) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }

    fn stop(&mut self) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }

    fn recv(&mut self) -> NetworkResult<Option<heapless::Vec<u8, 64>>> {
        Err(NetworkError::Other(0))
    }

    fn send(&mut self, data: &[u8]) -> NetworkResult<()> {
        Err(NetworkError::Other(0))
    }
}
