use crate::{errors::FSError, shared::*, NetworkError};
use alloc::{boxed::Box, rc::Rc, string::ToString};
use core::{cell::RefCell, marker::PhantomData, str};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{
    filesystem::ToShortFileName, LfnBuffer, Mode, RawDirectory, RawVolume, SdCard, ShortFileName,
    VolumeIdx, VolumeManager,
};
use esp_hal::{
    delay::Delay, gpio::Output, rng::Rng, spi::master::Spi, uart::Uart,
    usb_serial_jtag::UsbSerialJtag, Blocking,
};
use firefly_types::Encode;

type IoUart = Uart<'static, Blocking>;
type SdSpi = ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>;
type SD = SdCard<SdSpi, Delay>;
type VM = VolumeManager<SD, FakeTimesource, 48, 12, 1>;

pub struct DeviceImpl<'a> {
    delay: Delay,
    volume: RawVolume,
    vm: Rc<RefCell<VM>>,
    io_uart: Rc<RefCell<IoUart>>,
    usb_serial: Rc<RefCell<UsbSerialJtag<'static, Blocking>>>,
    addr: Addr,
    rng: Rng,
    _life: &'a PhantomData<()>,
}

impl DeviceImpl<'_> {
    pub fn new(
        sd_spi: SdSpi,
        io_uart: IoUart,
        usb_serial: UsbSerialJtag<'static, Blocking>,
        rng: Rng,
    ) -> Result<Self, NetworkError> {
        let sdcard = SdCard::new(sd_spi, Delay::new());
        let volume_manager: VM = VolumeManager::new_with_limits(sdcard, FakeTimesource {}, 5000);
        let Ok(volume) = volume_manager.open_volume(VolumeIdx(0)) else {
            return Err(NetworkError::Error("failed to open SD card volume 0"));
        };
        let volume = volume.to_raw_volume();

        let io_uart = Rc::new(RefCell::new(io_uart));
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
            vm: Rc::new(RefCell::new(volume_manager)),
            io_uart: io.uart,
            usb_serial: Rc::new(RefCell::new(usb_serial)),
            addr,
            rng,
            _life: &PhantomData,
        };
        Ok(device)
    }

    fn log(&self, msg: &str) {
        let msg = firefly_types::serial::Response::Log(msg.to_string());
        let raw = msg.encode_vec().unwrap();
        let n = cobs::max_encoding_length(raw.len());
        let mut buf = alloc::vec![0; n];
        cobs::encode(&raw, &mut buf);
        let mut usb = self.usb_serial.borrow_mut();
        // Non-blocking writes ensure that we won't block forever
        // if there is no client connected listening for logs.
        // However, that also means we might lose some logs
        // even if there is a client connected
        // (if the runtime writes faster than the client reads).
        for byte in &buf {
            _ = usb.write_byte_nb(*byte);
        }
        _ = usb.write_byte_nb(0x00);
        _ = usb.flush_tx_nb();
    }

    fn get_dir(&mut self, path: &[&str]) -> Result<RawDirectory, FSError> {
        let mut manager = self.vm.borrow_mut();
        let mut dir = manager.open_root_dir(self.volume)?;
        for part in path {
            let parent_dir = dir;
            dir = open_dir(&mut manager, dir, part)?;
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
    let short_name = get_short_name(manager, dir, name)?;
    Ok(manager.open_dir(dir, short_name)?)
}

fn get_short_name(manager: &VM, dir: RawDirectory, name: &str) -> Result<ShortFileName, FSError> {
    if let Ok(short_name) = name.to_short_filename() {
        return Ok(short_name);
    }
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
    let Some(file_name) = result else {
        return Err(FSError::NotFound);
    };
    Ok(file_name)
}

impl<'a> Device for DeviceImpl<'a> {
    type Read = FileR;
    type Write = FileW;
    type Network = NetworkImpl<'a>;
    type Serial = SerialImpl;

    fn now(&self) -> Instant {
        let now = esp_hal::time::Instant::now();
        Instant {
            us: now.duration_since_epoch().as_micros() as u32,
        }
    }

    fn delay(&self, d: Duration) {
        let d_micros = d.ns() / 1_000;
        let d = esp_hal::time::Duration::from_micros(d_micros as u64);
        self.delay.delay(d);
    }

    fn read_input(&mut self) -> Option<InputState> {
        use firefly_types::spi::*;
        let mut io = FireflyIO {
            uart: Rc::clone(&self.io_uart),
        };
        let req = Request::ReadInput;
        let Ok(raw) = io.transfer(req) else {
            // TODO: here and below, log the error
            return None;
        };
        let Ok(resp) = io.decode(&raw) else {
            return None;
        };
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
        let manager = &self.vm.borrow();
        let file_name = get_short_name(manager, dir, file_name)?;
        let file = manager.open_file_in_dir(dir, file_name, Mode::ReadOnly)?;
        _ = manager.close_dir(dir);
        Ok(FileR {
            vm: Rc::clone(&self.vm),
            file,
        })
    }

    fn create_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = &self.vm.borrow();
        let file_name = get_short_name(manager, dir, file_name)?;
        let file = manager.open_file_in_dir(dir, file_name, Mode::ReadWriteCreate)?;
        _ = manager.close_dir(dir);
        Ok(FileW {
            vm: Rc::clone(&self.vm),
            file,
        })
    }

    fn append_file(&mut self, path: &[&str]) -> Result<Self::Write, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = &self.vm.borrow();
        let file_name = get_short_name(manager, dir, file_name)?;
        let file = manager.open_file_in_dir(dir, file_name, Mode::ReadWriteAppend)?;
        _ = manager.close_dir(dir);
        Ok(FileW {
            vm: Rc::clone(&self.vm),
            file,
        })
    }

    fn get_file_size(&mut self, path: &[&str]) -> Result<u32, FSError> {
        let Some((file_name, dir_path)) = path.split_last() else {
            return Err(FSError::OpenedDirAsFile);
        };
        let dir = self.get_dir(dir_path)?;
        let manager = &self.vm.borrow();
        let file_name = get_short_name(manager, dir, file_name)?;
        let file = manager.open_file_in_dir(dir, file_name, Mode::ReadOnly)?;
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
        let manager = &self.vm.borrow();
        let file_name = get_short_name(manager, dir, file_name)?;
        manager.delete_file_in_dir(dir, file_name)?;
        _ = manager.close_dir(dir);
        Ok(())
    }

    fn iter_dir<F>(&mut self, path: &[&str], mut f: F) -> Result<(), FSError>
    where
        F: FnMut(crate::EntryKind, &[u8]),
    {
        let dir = self.get_dir(path)?;
        let manager = &self.vm.borrow();
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
                uart: Rc::clone(&self.io_uart),
            },
            addr: self.addr,
            _life: &PhantomData,
        }
    }

    fn serial(&self) -> Self::Serial {
        SerialImpl {
            usb_serial: Rc::clone(&self.usb_serial),
        }
    }

    fn has_headphones(&mut self) -> bool {
        false
    }

    fn get_audio_buffer(&mut self) -> &mut [i16] {
        &mut []
    }
}

pub struct FileW {
    vm: Rc<RefCell<VM>>,
    file: embedded_sdmmc::RawFile,
}

impl embedded_io::ErrorType for FileW {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Write for FileW {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let manager = &self.vm.borrow();
        match manager.write(self.file, buf) {
            Ok(()) => Ok(buf.len()),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        let manager = &self.vm.borrow();
        match manager.flush_file(self.file) {
            Ok(()) => Ok(()),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }
}

impl Drop for FileW {
    fn drop(&mut self) {
        let manager = &self.vm.borrow();
        _ = manager.close_file(self.file);
    }
}

pub struct FileR {
    vm: Rc<RefCell<VM>>,
    file: embedded_sdmmc::RawFile,
}

impl embedded_io::ErrorType for FileR {
    type Error = embedded_io::ErrorKind;
}

impl embedded_io::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let manager = &self.vm.borrow();
        match manager.read(self.file, buf) {
            Ok(size) => Ok(size),
            Err(_) => Err(embedded_io::ErrorKind::Other),
        }
    }
}

impl wasmi::Read for FileR {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, wasmi::errors::ReadError> {
        let manager = &self.vm.borrow();
        match manager.read(self.file, buf) {
            Ok(size) => Ok(size),
            Err(_) => Err(wasmi::errors::ReadError::UnknownError),
        }
    }
}

impl Drop for FileR {
    fn drop(&mut self) {
        let manager = &self.vm.borrow();
        _ = manager.close_file(self.file);
    }
}

struct FireflyIO {
    uart: Rc<RefCell<IoUart>>,
}

impl FireflyIO {
    fn transfer(
        &mut self,
        req: firefly_types::spi::Request<'_>,
    ) -> Result<alloc::vec::Vec<u8>, NetworkError> {
        let mut uart = self.uart.borrow_mut();

        // send request
        let mut raw = req.encode_vec()?;
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
        if let Response::Error(err) = resp {
            return Err(NetworkError::OwnedError(err.into()));
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

impl Network for NetworkImpl<'_> {
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

pub struct SerialImpl {
    usb_serial: Rc<RefCell<UsbSerialJtag<'static, Blocking>>>,
}

impl Serial for SerialImpl {
    fn start(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> NetworkResult<()> {
        Ok(())
    }

    fn recv(&mut self) -> NetworkResult<Option<Box<[u8]>>> {
        let mut usb = self.usb_serial.borrow_mut();
        let mut buf = alloc::vec::Vec::new();
        while let Ok(byte) = usb.read_byte() {
            buf.push(byte);
        }
        if buf.is_empty() {
            return Ok(None);
        }
        Ok(Some(buf.into_boxed_slice()))
    }

    fn send(&mut self, data: &[u8]) -> NetworkResult<()> {
        let mut usb = self.usb_serial.borrow_mut();
        // Non-blocking writes ensure that we won't block forever
        // if there is no client connected listening for messages.
        // However, that also means we might lose some messages
        // even if there is a client connected
        // (if the runtime writes faster than the client reads).
        for byte in data {
            _ = usb.write_byte_nb(*byte);
        }
        _ = usb.write_byte_nb(0x00);
        _ = usb.flush_tx_nb();
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
