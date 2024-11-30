use crate::shared::{Device, Network, Serial};
use esp_hal::delay::Delay;
use fugit::MicrosDurationU64;

pub struct DeviceImpl {
    delay: Delay,
}

impl Device for DeviceImpl {
    type Read = File;
    type Write = File;
    type Network = NetworkImpl;
    type Serial = SerialImpl;

    fn now(&self) -> crate::Instant {
        todo!()
    }

    fn delay(&self, d: crate::Duration) {
        let d_micros = d.ns() / 1_000;
        let d = MicrosDurationU64::from_ticks(d_micros as u64);
        self.delay.delay(d);
    }

    fn read_input(&mut self) -> Option<crate::InputState> {
        todo!()
    }

    fn log_debug<D: core::fmt::Display>(&self, src: &str, msg: D) {
        todo!()
    }

    fn log_error<D: core::fmt::Display>(&self, src: &str, msg: D) {
        todo!()
    }

    fn open_file(&self, path: &[&str]) -> Option<Self::Read> {
        todo!()
    }

    fn create_file(&self, path: &[&str]) -> Option<Self::Write> {
        todo!()
    }

    fn append_file(&self, path: &[&str]) -> Option<Self::Write> {
        todo!()
    }

    fn get_file_size(&self, path: &[&str]) -> Option<u32> {
        todo!()
    }

    fn make_dir(&self, path: &[&str]) -> bool {
        todo!()
    }

    fn remove_file(&self, path: &[&str]) -> bool {
        todo!()
    }

    fn iter_dir<F>(&self, path: &[&str], f: F) -> bool
    where
        F: FnMut(crate::EntryKind, &[u8]),
    {
        todo!()
    }

    fn network(&self) -> Self::Network {
        todo!()
    }

    fn serial(&self) -> Self::Serial {
        todo!()
    }

    fn has_headphones(&mut self) -> bool {
        todo!()
    }

    fn get_audio_buffer(&mut self) -> &mut [i16] {
        todo!()
    }
}

pub struct File {}

impl embedded_io::ErrorType for File {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        todo!()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        todo!()
    }
}

impl embedded_io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        todo!()
    }
}

impl wasmi::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        todo!()
    }
}

pub struct NetworkImpl {}

impl Network for NetworkImpl {
    type Addr = ();

    fn start(&mut self) -> crate::NetworkResult<()> {
        todo!()
    }

    fn stop(&mut self) -> crate::NetworkResult<()> {
        todo!()
    }

    fn local_addr(&self) -> Self::Addr {
        todo!()
    }

    fn advertise(&mut self) -> crate::NetworkResult<()> {
        todo!()
    }

    fn recv(&mut self) -> crate::NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>> {
        todo!()
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> crate::NetworkResult<()> {
        todo!()
    }
}

pub struct SerialImpl {}

impl Serial for SerialImpl {
    fn start(&mut self) -> crate::NetworkResult<()> {
        todo!()
    }

    fn stop(&mut self) -> crate::NetworkResult<()> {
        todo!()
    }

    fn recv(&mut self) -> crate::NetworkResult<Option<heapless::Vec<u8, 64>>> {
        todo!()
    }

    fn send(&mut self, data: &[u8]) -> crate::NetworkResult<()> {
        todo!()
    }
}
