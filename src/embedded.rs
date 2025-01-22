use crate::{errors::FSError, shared::*, NetworkError};
use alloc::boxed::Box;
use core::{cell::OnceCell, marker::PhantomData, str};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{
    filesystem::ToShortFileName, LfnBuffer, Mode, RawDirectory, RawVolume, SdCard, VolumeIdx,
    VolumeManager,
};
use esp_hal::{
    delay::Delay, gpio::Output, rng::Rng, spi::master::Spi, timer::systimer::SystemTimer,
    uart::Uart, Blocking,
};
use firefly_types::Encode;
use fugit::MicrosDurationU64;

type IoUart = Uart<'static, Blocking>;
type SdSpi = ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>;
type SD = SdCard<SdSpi, Delay>;
type VM = VolumeManager<SD, FakeTimesource, 48, 12, 1>;
static mut VOLUME_MANAGER: OnceCell<VM> = OnceCell::new();

fn get_volume_manager() -> &'static mut VM {
    unsafe { VOLUME_MANAGER.get_mut() }.unwrap()
}

pub struct DeviceImpl<'a> {
    delay: Delay,
    volume: RawVolume,
    io_uart: Option<IoUart>,
    addr: Addr,
    rng: Rng,
    _life: &'a PhantomData<()>,
}

impl<'a> DeviceImpl<'a> {
    pub fn new(sd_spi: SdSpi, io_uart: IoUart, rng: Rng) -> Result<Self, NetworkError> {
        let sdcard = SdCard::new(sd_spi, Delay::new());
        let volume_manager: VM = VolumeManager::new_with_limits(sdcard, FakeTimesource {}, 5000);
        let volume = volume_manager
            .open_volume(VolumeIdx(0))
            .unwrap()
            .to_raw_volume();
        let res = unsafe { VOLUME_MANAGER.set(volume_manager) };
        if res.is_err() {
            return Err(NetworkError::AlreadyInitialized);
        }

        let mut io = FireflyIO { uart: io_uart };
        let req = firefly_types::spi::Request::NetLocalAddr;
        let raw = io.transfer(req)?;
        let resp = io.decode(&raw)?;
        let addr = match resp {
            firefly_types::spi::Response::NetLocalAddr(addr) => addr,
            _ => return Err(NetworkError::UnexpectedResp),
        };

        let device = Self {
            delay: Delay::new(),
            volume,
            io_uart: Some(io.uart),
            addr,
            rng,
            _life: &PhantomData,
        };
        Ok(device)
    }

    fn log(&self, msg: &str) {
        esp_println::println!("{msg}");
        // let mut uart = self.uart.replace(None);
        // _ = uart.as_mut().unwrap().write_bytes(msg.as_bytes());
        // self.uart.replace(uart);
    }

    fn get_dir(&mut self, path: &[&str]) -> Result<RawDirectory, FSError> {
        let manager = get_volume_manager();
        let mut dir = manager.open_root_dir(self.volume)?;
        for part in path {
            let parent_dir = dir;
            dir = open_dir(manager, dir, part)?;
            _ = manager.close_dir(parent_dir);
        }
        Ok(dir)
    }
}

/// Open directory with the given name.
///
/// If the name is a valid FAT-16 short name, use that name directly.
/// Otherwise, iterate through all items in the directory, find an entry
/// with the given long name, get its short name, and use that to open the directory.
fn open_dir(manager: &mut VM, dir: RawDirectory, name: &str) -> Result<RawDirectory, FSError> {
    let short_name = match name.to_short_filename() {
        Ok(short_name) => short_name,
        Err(_) => {
            let mut result = None;
            let mut buf = [0u8; 64];
            let mut lfnb = LfnBuffer::new(&mut buf);
            manager.iterate_dir_lfn(dir, &mut lfnb, |entry, long_name| {
                if result.is_some() {
                    return;
                }
                let Some(long_name) = long_name else { return };
                if long_name.trim_ascii() == name {
                    result = Some(entry.name.clone())
                }
            })?;
            let Some(dir) = result else {
                return Err(FSError::NotFound);
            };
            dir
        }
    };
    Ok(manager.open_dir(dir, short_name)?)
}

impl<'a> Device for DeviceImpl<'a> {
    type Read = FileR;
    type Write = FileW;
    type Network = NetworkImpl<'a>;
    type Serial = SerialImpl;

    fn now(&self) -> Instant {
        debug_assert_eq!(SystemTimer::ticks_per_second(), 16_000_000);
        Instant {
            us: (SystemTimer::now() / 16) as u32,
        }
    }

    fn delay(&self, d: Duration) {
        let d_micros = d.ns() / 1_000;
        let d = MicrosDurationU64::from_ticks(d_micros as u64);
        self.delay.delay(d);
    }

    fn read_input(&mut self) -> Option<InputState> {
        use firefly_types::spi::*;
        let mut io = FireflyIO {
            uart: self.io_uart.take().unwrap(),
        };
        let req = Request::ReadInput;
        let Ok(raw) = io.transfer(req) else {
            // TODO: here and below, log the error
            self.io_uart = Some(io.uart);
            return None;
        };
        let Ok(resp) = io.decode(&raw) else {
            self.io_uart = Some(io.uart);
            return None;
        };
        self.io_uart = Some(io.uart);
        match resp {
            Response::Input(pad, buttons) => Some(InputState {
                pad: pad.map(|(x, y)| Pad { x, y: -y }),
                buttons: !buttons,
            }),
            // Response::PadError => None,
            _ => None,
        }
    }

    fn log_debug<D: core::fmt::Display>(&self, src: &str, msg: D) {
        let msg = alloc::format!("DEBUG({src}): {msg}");
        self.log(&msg);
    }

    fn log_error<D: core::fmt::Display>(&self, src: &str, msg: D) {
        let msg = alloc::format!("ERROR({src}): {msg}");
        self.log(&msg);
    }

    fn random(&mut self) -> u32 {
        self.rng.random()
    }

    fn open_file(&mut self, path: &[&str]) -> Result<Self::Read, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadOnly)?;
        _ = manager.close_dir(dir);
        Ok(FileR { file })
    }

    fn create_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadWriteCreate)?;
        _ = manager.close_dir(dir);
        Ok(FileW { file })
    }

    fn append_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        let file = manager.open_file_in_dir(dir, *file_name, Mode::ReadWriteAppend)?;
        _ = manager.close_dir(dir);
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
        _ = manager.close_dir(dir);
        Ok(size)
    }

    fn remove_file(&mut self, path: &[&str]) -> Result<(), FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = get_volume_manager();
        manager.delete_file_in_dir(dir, *file_name)?;
        _ = manager.close_dir(dir);
        Ok(())
    }

    fn iter_dir<F>(&mut self, path: &[&str], mut f: F) -> Result<(), FSError>
    where
        F: FnMut(crate::EntryKind, &[u8]),
    {
        let dir = self.get_dir(path)?;
        let manager = get_volume_manager();
        let mut buf = [0u8; 64];
        let mut lfnb = LfnBuffer::new(&mut buf);
        manager.iterate_dir_lfn(dir, &mut lfnb, |entry, long_name| {
            let base_name = entry.name.base_name();
            if base_name.first() == Some(&b'.') {
                return;
            }
            let name = match long_name {
                Some(long_name) => long_name.trim_ascii().as_bytes(),
                None => base_name,
            };
            let kind = if entry.attributes.is_directory() {
                EntryKind::Dir
            } else {
                EntryKind::File
            };
            f(kind, name);
        })?;
        _ = manager.close_dir(dir);
        Ok(())
    }

    fn network(&mut self) -> Self::Network {
        NetworkImpl {
            io: FireflyIO {
                uart: self.io_uart.take().unwrap(),
            },
            addr: self.addr,
            _life: &PhantomData,
        }
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

impl Drop for FileW {
    fn drop(&mut self) {
        let manager = get_volume_manager();
        _ = manager.close_file(self.file);
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

impl Drop for FileR {
    fn drop(&mut self) {
        let manager = get_volume_manager();
        _ = manager.close_file(self.file);
    }
}

struct FireflyIO {
    uart: IoUart,
}

impl FireflyIO {
    fn transfer(
        &mut self,
        req: firefly_types::spi::Request<'_>,
    ) -> Result<alloc::vec::Vec<u8>, NetworkError> {
        let uart = &mut self.uart;

        // send request
        let mut raw = req.encode_vec().unwrap();
        let Ok(size) = u8::try_from(raw.len()) else {
            return Err(NetworkError::Error("request payload is too big"));
        };
        uart.write_bytes(&[size])?;
        uart.write_bytes(&raw[..])?;

        // read response
        uart.read_bytes(&mut raw[..1])?;
        let size = usize::from(raw[0]);
        if size == 0 {
            return Err(NetworkError::Error("received zero-sized message"));
        }
        raw.resize(size, 0);
        uart.read_bytes(&mut raw[..])?;
        Ok(raw)
    }

    fn decode<'b>(&self, raw: &'b [u8]) -> NetworkResult<firefly_types::spi::Response<'b>> {
        use firefly_types::spi::Response;
        if raw.is_empty() {
            return Err(NetworkError::Error("buffer is empty, cannot decode"));
        }
        let resp = Response::decode(raw)?;
        if let Response::NetError(err) = resp {
            return Err(NetworkError::Other(err));
        }
        Ok(resp)
    }
}

pub struct NetworkImpl<'a> {
    io: FireflyIO,
    addr: Addr,
    _life: &'a PhantomData<()>,
}

pub type Addr = [u8; 6];

impl<'a> Network for NetworkImpl<'a> {
    type Addr = Addr;

    fn start(&mut self) -> NetworkResult<()> {
        let req = firefly_types::spi::Request::NetStart;
        let raw = self.io.transfer(req)?;
        let resp = self.io.decode(&raw)?;
        if resp != firefly_types::spi::Response::NetStarted {
            return Err(NetworkError::UnexpectedResp);
        }
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        let req = firefly_types::spi::Request::NetStop;
        let raw = self.io.transfer(req)?;
        let resp = self.io.decode(&raw)?;
        if resp != firefly_types::spi::Response::NetStopped {
            return Err(NetworkError::UnexpectedResp);
        }
        Ok(())
    }

    fn local_addr(&self) -> Self::Addr {
        self.addr
    }

    fn advertise(&mut self) -> NetworkResult<()> {
        let req = firefly_types::spi::Request::NetAdvertise;
        let raw = self.io.transfer(req)?;
        let resp = self.io.decode(&raw)?;
        if resp != firefly_types::spi::Response::NetAdvertised {
            return Err(NetworkError::UnexpectedResp);
        }
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<(Self::Addr, Box<[u8]>)>> {
        let req = firefly_types::spi::Request::NetRecv;
        let raw = self.io.transfer(req)?;
        let resp = self.io.decode(&raw)?;
        use firefly_types::spi::Response::*;
        match resp {
            NetIncoming(addr, msg) => {
                let msg = msg.to_vec().into_boxed_slice();
                Ok(Some((addr, msg)))
            }
            NetNoIncoming => Ok(None),
            _ => Err(NetworkError::UnexpectedResp),
        }
    }

    fn send(&mut self, addr: Self::Addr, data: &[u8]) -> NetworkResult<()> {
        let req = firefly_types::spi::Request::NetSend(addr, data);
        let raw = self.io.transfer(req)?;
        let resp = self.io.decode(&raw)?;
        if resp != firefly_types::spi::Response::NetSent {
            return Err(NetworkError::UnexpectedResp);
        }
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

    fn recv(&mut self) -> NetworkResult<Option<Box<[u8]>>> {
        Ok(None)
    }

    fn send(&mut self, _data: &[u8]) -> NetworkResult<()> {
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
