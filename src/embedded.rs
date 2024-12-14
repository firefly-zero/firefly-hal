use crate::{errors::FSError, shared::*};
use core::cell::OnceCell;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embedded_sdmmc::{Mode, RawVolume, SdCard, VolumeIdx, VolumeManager};
use esp_hal::{
    delay::Delay, gpio::Output, spi::master::Spi, timer::systimer::SystemTimer, Blocking,
};
use fugit::MicrosDurationU64;

type SD = SdCard<ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, NoDelay>, Delay>;
type VM = VolumeManager<SD, FakeTimesource, 48, 12, 1>;
static mut VOLUME_MANAGER: OnceCell<VM> = OnceCell::new();
static mut VOLUME: Option<RawVolume> = None;

fn get_volume_manager() -> &'static mut VM {
    unsafe { VOLUME_MANAGER.get_mut() }.unwrap()
}

fn get_volume() -> RawVolume {
    unsafe { VOLUME.unwrap() }
}

pub struct DeviceImpl {
    delay: Delay,
}

impl DeviceImpl {
    pub fn new(sdcard: SD) -> Result<Self, esp_hal::uart::Error> {
        let volume_manager: VM = VolumeManager::new_with_limits(sdcard, FakeTimesource {}, 5000);
        let volume = volume_manager
            .open_volume(VolumeIdx(0))
            .unwrap()
            .to_raw_volume();
        unsafe { VOLUME = Some(volume) };
        unsafe { VOLUME_MANAGER.set(volume_manager) }.ok().unwrap();
        let device = Self {
            delay: Delay::new(),
        };
        Ok(device)
    }

    fn log(&self, msg: &str) {
        esp_println::println!("{msg}");
        // let mut uart = self.uart.replace(None);
        // _ = uart.as_mut().unwrap().write_bytes(msg.as_bytes());
        // self.uart.replace(uart);
    }

    fn get_dir(&mut self, path: &[&str]) -> Result<embedded_sdmmc::RawDirectory, FSError> {
        let manager = get_volume_manager();
        let volume = get_volume();
        let mut dir = manager.open_root_dir(volume)?;
        for part in path {
            dir = manager.open_dir(dir, *part)?;
        }
        Ok(dir)
    }
}

impl Device for DeviceImpl {
    type Read = FileR;
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

    fn open_file(&mut self, path: &[&str]) -> Result<Self::Read, FSError> {
        // self.flash.read(offset, bytes);
        // match path {
        //     ["roms", "demo", "go-triangle", "_bin"] => Some(FileR { bin: BIN }),
        //     ["roms", "demo", "go-triangle", "_meta"] => Some(FileR { bin: META }),
        //     _ => None,
        // }

        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadOnly)?;
        Ok(FileR { file })
    }

    fn create_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadWriteCreate)?;
        Ok(FileW { file })
    }

    fn append_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadWriteAppend)?;
        Ok(FileW { file })
    }

    fn get_file_size(&mut self, path: &[&str]) -> Result<u32, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadOnly)?;
        let size = manager.file_length(file)?;
        _ = manager.close_file(file);
        Ok(size)
    }

    fn remove_file(&mut self, path: &[&str]) -> Result<(), FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        manager.delete_file_in_dir(dir, *file_name)?;
        Ok(())
    }

    fn iter_dir<F>(&mut self, path: &[&str], mut f: F) -> Result<(), FSError>
    where
        F: FnMut(crate::EntryKind, &[u8]),
    {
        let dir = self.get_dir(path)?;
        let manager = get_volume_manager();
        manager.iterate_dir(dir, |entry| {
            let name = entry.name.base_name();
            f(EntryKind::File, name);
        })?;
        Ok(())
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

pub struct FileW {
    file: embedded_sdmmc::RawFile,
}

impl embedded_io::ErrorType for FileW {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Write for FileW {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let manager = get_volume_manager();
        match manager.write(self.file, buf) {
            Ok(()) => Ok(buf.len()),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let manager = get_volume_manager();
        match manager.flush_file(self.file) {
            Ok(()) => Ok(()),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }
}

pub struct FileR {
    file: embedded_sdmmc::RawFile,
}

impl embedded_io::ErrorType for FileR {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let manager = get_volume_manager();
        match manager.read(self.file, buf) {
            Ok(size) => Ok(size),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }
}

impl wasmi::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        let manager = get_volume_manager();
        match manager.read(self.file, buf) {
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
