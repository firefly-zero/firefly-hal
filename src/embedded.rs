use crate::{NetworkError, errors::FSError, shared::*};
use alloc::{boxed::Box, rc::Rc, string::ToString, vec::Vec};
use core::{cell::RefCell, marker::PhantomData, str};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_io::Read;
use embedded_sdmmc::{
    LfnBuffer, Mode, RawDirectory, RawFile, RawVolume, SdCard, ShortFileName, VolumeIdx,
    VolumeManager, filesystem::ToShortFileName,
};
use esp_hal::{
    Blocking, delay::Delay, gpio::Output, rng::Rng, spi::master::Spi, uart::Uart,
    usb_serial_jtag::UsbSerialJtag,
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
        let mut usb = self.usb_serial.borrow_mut();
        send_to_serial(&mut usb, &raw);
    }

    pub fn alloc_psram(&self, size: usize) -> Vec<u8, esp_alloc::ExternalMemory> {
        Vec::with_capacity_in(size, esp_alloc::ExternalMemory)
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

/// Open a file in a dir and close the dir. Long file names are supported.
fn open_file(
    manager: &VM,
    dir: RawDirectory,
    file_name: &str,
    mode: Mode,
) -> Result<RawFile, FSError> {
    let short_name = match get_short_name(manager, dir, file_name) {
        Ok(short_name) => short_name,
        Err(err) => {
            _ = manager.close_dir(dir);
            return Err(err);
        }
    };
    let res = manager.open_file_in_dir(dir, short_name, mode);
    let file = res?;
    Ok(file)
}

impl<'a> Device for DeviceImpl<'a> {
    type Network = NetworkImpl<'a>;
    type Serial = SerialImpl;
    type Dir = DirImpl;

    fn now(&self) -> Instant {
        let now = esp_hal::time::Instant::now();
        Instant {
            us: now.duration_since_epoch().as_micros() as u32,
        }
    }

    fn delay(&self, d: Duration) {
        let d = esp_hal::time::Duration::from_micros(d.us() as u64);
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
                pad: pad.map(format_pad),
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

    fn open_dir(&mut self, path: &[&str]) -> Result<DirImpl, FSError> {
        let mut manager = self.vm.borrow_mut();
        let mut dir = manager.open_root_dir(self.volume)?;
        for part in path {
            let open_res = open_dir(&mut manager, dir, part);
            _ = manager.close_dir(dir);
            dir = open_res?;
        }
        Ok(DirImpl {
            dir,
            vm: self.vm.clone(),
        })
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

    fn get_battery_status(&mut self) -> Option<BatteryStatus> {
        Some(BatteryStatus {
            voltage: 50,
            connected: true,
            full: false,
        })
    }
}

pub struct DirImpl {
    vm: Rc<RefCell<VM>>,
    dir: RawDirectory,
}

impl Dir for DirImpl {
    type Read = FileR;
    type Write = FileW;

    fn open_file(&mut self, name: &str) -> Result<Self::Read, FSError> {
        let manager = &self.vm.borrow();
        let file = open_file(manager, self.dir, name, Mode::ReadOnly)?;
        Ok(FileR {
            vm: Rc::clone(&self.vm),
            file,
        })
    }

    fn create_file(&mut self, name: &str) -> Result<Self::Write, FSError> {
        let manager = &self.vm.borrow();
        let file = open_file(manager, self.dir, name, Mode::ReadWriteCreateOrTruncate)?;
        Ok(FileW {
            vm: Rc::clone(&self.vm),
            file,
        })
    }

    fn append_file(&mut self, name: &str) -> Result<Self::Write, FSError> {
        let manager = &self.vm.borrow();
        let file = open_file(manager, self.dir, name, Mode::ReadWriteAppend)?;
        Ok(FileW {
            vm: Rc::clone(&self.vm),
            file,
        })
    }

    fn get_file_size(&mut self, name: &str) -> Result<u32, FSError> {
        let manager = &self.vm.borrow();
        let file = open_file(manager, self.dir, name, Mode::ReadOnly)?;
        let size = manager.file_length(file)?;
        _ = manager.close_file(file);
        Ok(size)
    }

    fn remove_file(&mut self, name: &str) -> Result<(), FSError> {
        let manager = &self.vm.borrow();
        let short_name = match get_short_name(manager, self.dir, name) {
            Ok(short_name) => short_name,
            Err(err) => {
                return Err(err);
            }
        };
        let res = manager.delete_file_in_dir(self.dir, short_name);
        res?;
        Ok(())
    }

    fn iter_dir<F>(&mut self, mut f: F) -> Result<(), FSError>
    where
        F: FnMut(crate::EntryKind, &[u8]),
    {
        let manager = &self.vm.borrow();
        let mut buf = [0u8; 64];
        let mut lfnb = LfnBuffer::new(&mut buf);
        manager.iterate_dir_lfn(self.dir, &mut lfnb, |entry, long_name| {
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
        Ok(())
    }
}

impl Drop for DirImpl {
    fn drop(&mut self) {
        let manager = &self.vm.borrow();
        _ = manager.close_dir(self.dir);
    }
}

fn format_pad(raw: (u16, u16)) -> Pad {
    use micromath::F32;

    // The minimum values are picked empirically to remove
    // dead zones on the left and on the top.
    const X_MIN: u16 = 50;
    const Y_MIN: u16 = 50;

    // The maximum possible values
    // according to the touchpad's datasheet.
    const X_MAX: u16 = 2047;
    const Y_MAX: u16 = 1535;

    let raw_x = raw.0;
    let raw_y = raw.1;

    // Remove dead zone on the left.
    let raw_x = raw_x.saturating_sub(X_MIN);
    let raw_y = raw_y.saturating_sub(Y_MIN);

    // Project on the range -1.0..=1.0.
    let x = F32::from(raw_x * 2) / F32::from(X_MAX - X_MIN) - 1.;
    let y = F32::from(raw_y * 2) / F32::from(Y_MAX - Y_MIN) - 1.;

    // Scale to remove dead zones on the sides.
    // The scale values are picked empirically.
    let mut x = x * 1.40;
    let mut y = y * 1.25;

    // Scaling might result in the dot being out of circle.
    // If so, project it back to the circle.
    let square = x.mul_add(x, y * y); // x²+y²
    if square >= 1. {
        let descale = square.sqrt();
        x /= descale;
        y /= descale;
    }

    // Project on the range -1000.=1000.
    let x = f32::from(x * 1000.) as i16;
    let y = f32::from(y * -1000.) as i16;
    Pad { x, y }
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
    /// Send request and read response.
    fn transfer(&mut self, req: firefly_types::spi::Request<'_>) -> Result<Vec<u8>, NetworkError> {
        let mut uart = self.uart.borrow_mut();

        // send request
        let mut raw = req.encode_vec()?;
        let Ok(size) = u8::try_from(raw.len()) else {
            return Err(NetworkError::Error("request payload is too big"));
        };
        uart.write(&[size])?;
        uart.write(&raw[..])?;

        // read response
        uart.read(&mut raw[..1])?;
        let size = usize::from(raw[0]);
        if size == 0 {
            return Err(NetworkError::Error("received zero-sized message"));
        }
        raw.resize(size, 0);
        uart.read_exact(&mut raw[..])?;
        Ok(raw)
    }

    /// Send request without reading response.
    fn send(&mut self, req: firefly_types::spi::Request<'_>) -> Result<(), NetworkError> {
        let mut uart = self.uart.borrow_mut();
        let raw = req.encode_vec()?;
        let Ok(size) = u8::try_from(raw.len()) else {
            return Err(NetworkError::Error("request payload is too big"));
        };
        uart.write(&[size])?;
        uart.write(&raw[..])?;
        Ok(())
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
        self.io.send(req)?;
        Ok(())
    }

    fn send_status(&mut self, addr: Self::Addr) -> NetworkResult<firefly_types::spi::SendStatus> {
        let req = firefly_types::spi::Request::NetSendStatus(addr);
        let raw = self.io.transfer(req)?;
        let resp = self.io.decode(&raw)?;
        use firefly_types::spi::Response::*;
        match resp {
            NetSendStatus(status) => Ok(status),
            _ => Err(NetworkError::UnexpectedResp),
        }
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
        let mut buf = Vec::new();
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
        send_to_serial(&mut usb, data);
        Ok(())
    }
}

fn send_to_serial(usb: &mut UsbSerialJtag<'static, Blocking>, data: &[u8]) {
    let n = cobs::max_encoding_length(data.len());
    let mut buf = alloc::vec![0; n];
    cobs::encode(data, &mut buf);
    // Non-blocking writes ensure that we won't block forever
    // if there is no client connected listening for messages.
    // However, that also means we might lose some messages
    // even if there is a client connected
    // (if the runtime writes faster than the client reads).
    for byte in buf {
        _ = usb.write_byte_nb(byte);
    }
    _ = usb.write_byte_nb(0x00);
    _ = usb.flush_tx_nb();
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
