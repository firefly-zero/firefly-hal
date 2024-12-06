use crate::shared::*;
use core::cell::Cell;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embedded_io::Write;
use embedded_sdmmc::{Mode, SdCard, VolumeIdx, VolumeManager};
use embedded_storage::{ReadStorage, Storage};
use esp_hal::{
    delay::Delay, gpio::Output, spi::master::Spi, timer::systimer::SystemTimer, Blocking,
};
use esp_storage::FlashStorage;
use fugit::MicrosDurationU64;

static BIN: &[u8] = include_bytes!("/home/gram/.local/share/firefly/roms/demo/go-triangle/_bin");
static META: &[u8] = include_bytes!("/home/gram/.local/share/firefly/roms/demo/go-triangle/_meta");

type SD = SdCard<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, NoDelay>, Delay>;

pub struct DeviceImpl {
    delay: Delay,
    // uart: Cell<Option<Uart<'static, Blocking>>>,
    volume_manager: VolumeManager<SD, FakeTimesource>,
}

impl DeviceImpl {
    pub fn new(sdcard: SD) -> Result<Self, esp_hal::uart::Error> {
        let volume_manager = embedded_sdmmc::VolumeManager::new(sdcard, FakeTimesource {});
        let device = Self {
            delay: Delay::new(),
            // uart: Cell::new(Some(uart)),
            volume_manager,
        };
        Ok(device)
    }

    fn log(&self, msg: &str) {
        esp_println::println!("{msg}");
        // let mut uart = self.uart.replace(None);
        // _ = uart.as_mut().unwrap().write_bytes(msg.as_bytes());
        // self.uart.replace(uart);
    }

    fn get_dir(&mut self, path: &[&str]) -> Option<embedded_sdmmc::RawDirectory> {
        let manager = &mut self.volume_manager;
        let volume0 = manager.open_volume(VolumeIdx(0)).ok()?;
        let volume0 = volume0.to_raw_volume();
        let mut dir = manager.open_root_dir(volume0).ok()?;
        for part in path {
            dir = manager.open_dir(dir, *part).ok()?;
        }
        Some(dir)
    }
}

impl<'a> Device<'a> for DeviceImpl {
    type Read = FileR<'a>;
    type Write = FileW;
    type Network = NetworkImpl;
    type Serial = SerialImpl;

    fn now(&self) -> Instant {
        debug_assert_eq!(SystemTimer::ticks_per_second(), 16_000_000);
        Instant {
            ns: (SystemTimer::now() * 125 / 2) as u32,
        }
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

    fn open_file(&'a mut self, path: &[&str]) -> Option<Self::Read> {
        // self.flash.read(offset, bytes);
        // match path {
        //     ["roms", "demo", "go-triangle", "_bin"] => Some(FileR { bin: BIN }),
        //     ["roms", "demo", "go-triangle", "_meta"] => Some(FileR { bin: META }),
        //     _ => None,
        // }

        let (file_name, dir_path) = path.split_last()?;
        let dir = self.get_dir(dir_path)?;
        let file = self
            .volume_manager
            .open_file_in_dir(dir, *file_name, Mode::ReadOnly)
            .ok()?;
        Some(FileR {
            volume_manager: &mut self.volume_manager,
            file,
        })
    }

    fn create_file(&mut self, path: &[&str]) -> Option<Self::Write> {
        None
    }

    fn append_file(&mut self, path: &[&str]) -> Option<Self::Write> {
        None
    }

    fn get_file_size(&mut self, path: &[&str]) -> Option<u32> {
        let (file_name, dir_path) = path.split_last()?;
        let dir = self.get_dir(dir_path)?;
        let file = self
            .volume_manager
            .open_file_in_dir(dir, *file_name, Mode::ReadOnly)
            .ok()?;
        self.volume_manager.file_length(file).ok()
    }

    fn make_dir(&mut self, path: &[&str]) -> bool {
        let manager = &mut self.volume_manager;
        let Ok(volume0) = manager.open_volume(VolumeIdx(0)) else {
            return false;
        };
        let volume0 = volume0.to_raw_volume();
        let Ok(mut dir) = manager.open_root_dir(volume0) else {
            return false;
        };
        for part in path {
            let Ok(_) = manager.make_dir_in_dir(dir, *part) else {
                return false;
            };
            let Ok(new_dir) = manager.open_dir(dir, *part) else {
                return false;
            };
            dir = new_dir;
        }
        true
    }

    fn remove_file(&mut self, path: &[&str]) -> bool {
        false
    }

    fn iter_dir<F>(&mut self, path: &[&str], f: F) -> bool
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

pub struct FileR<'a> {
    volume_manager: &'a mut VolumeManager<SD, FakeTimesource>,
    file: embedded_sdmmc::RawFile,
}

impl<'a> embedded_io::ErrorType for FileR<'a> {
    type Error = embedded_io::ErrorKind;
}

impl<'a> embedded_io::Read for FileR<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self.volume_manager.read(self.file, buf) {
            Ok(size) => Ok(size),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }
}

impl<'a> wasmi::Read for FileR<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        match self.volume_manager.read(self.file, buf) {
            Ok(size) => Ok(size),
            Err(_) => Err(wasmi::errors::ReadError::UnknownError),
        }
    }
}

pub struct NetworkImpl {}

impl Network for NetworkImpl {
    type Addr = ();

    fn start(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn local_addr(&self) -> Self::Addr {}

    fn advertise(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, heapless::Vec<u8, 64>)>> {
        Ok(None)
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()> {
        Ok(())
    }
}

pub struct SerialImpl {}

impl Serial for SerialImpl {
    fn start(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<heapless::Vec<u8, 64>>> {
        Ok(None)
    }

    fn send(&mut self, data: &[u8]) -> NetworkResult<()> {
        Ok(())
    }
}

struct FakeTimesource {}

impl embedded_sdmmc::TimeSource for FakeTimesource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        embedded_sdmmc::Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}
