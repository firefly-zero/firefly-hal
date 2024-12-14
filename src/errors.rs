use core::fmt;

pub enum FSError {
    /// The underlying block device threw an error.
    DeviceError(alloc::string::String),
    /// The filesystem is badly formatted (or this code is buggy).
    FormatError(&'static str),
    /// The given `VolumeIdx` was bad,
    NoSuchVolume,
    /// The given filename was bad
    FilenameError(embedded_sdmmc::FilenameError),
    /// Out of memory opening volumes
    TooManyOpenVolumes,
    /// Out of memory opening directories
    TooManyOpenDirs,
    /// Out of memory opening files
    TooManyOpenFiles,
    /// Bad handle given
    BadHandle,
    /// That file or directory doesn't exist
    NotFound,
    /// You can't open a file twice or delete an open file
    FileAlreadyOpen,
    /// You can't open a directory twice
    DirAlreadyOpen,
    /// You can't open a directory as a file
    OpenedDirAsFile,
    /// You can't open a file as a directory
    OpenedFileAsDir,
    /// You can't delete a directory as a file
    DeleteDirAsFile,
    /// You can't close a volume with open files or directories
    VolumeStillInUse,
    /// You can't open a volume twice
    VolumeAlreadyOpen,
    /// We can't do that yet
    Unsupported,
    /// Tried to read beyond end of file
    EndOfFile,
    /// Found a bad cluster
    BadCluster,
    /// Error while converting types
    ConversionError,
    /// The device does not have enough space for the operation
    NotEnoughSpace,
    /// Cluster was not properly allocated by the library
    AllocationError,
    /// Jumped to free space during FAT traversing
    UnterminatedFatChain,
    /// Tried to open Read-Only file with write mode
    ReadOnly,
    /// Tried to create an existing file
    FileAlreadyExists,
    /// Bad block size - only 512 byte blocks supported
    BadBlockSize(u16),
    /// Bad offset given when seeking
    InvalidOffset,
    /// Disk is full
    DiskFull,
    /// A directory with that name already exists
    DirAlreadyExists,
    // The filesystem tried to gain a lock whilst already locked.
    Deadlock,

    /// The operation lacked the necessary privileges to complete.
    PermissionDenied,
    /// The connection was refused by the remote server.
    ConnectionRefused,
    /// The connection was reset by the remote server.
    ConnectionReset,
    /// The connection was aborted (terminated) by the remote server.
    ConnectionAborted,
    /// The network operation failed because it was not connected yet.
    NotConnected,
    /// A socket address could not be bound because the address is already in
    /// use elsewhere.
    AddrInUse,
    /// A nonexistent interface was requested or the requested address was not
    /// local.
    AddrNotAvailable,
    /// The operation failed because a pipe was closed.
    BrokenPipe,
    /// A parameter was incorrect.
    InvalidInput,
    /// Data not valid for the operation were encountered.
    ///
    /// Unlike [`InvalidInput`], this typically means that the operation
    /// parameters were valid, however the error was caused by malformed
    /// input data.
    ///
    /// For example, a function that reads a file into a string will error with
    /// `InvalidData` if the file's contents are not valid UTF-8.
    ///
    /// [`InvalidInput`]: ErrorKind::InvalidInput
    InvalidData,
    /// The I/O operation's timeout expired, causing it to be canceled.
    TimedOut,
    /// This operation was interrupted.
    ///
    /// Interrupted operations can typically be retried.
    Interrupted,
    /// An attempted write could not write any data.
    WriteZero,

    /// Something else.
    Other,
}

#[cfg(target_os = "none")]
impl<T: fmt::Debug> From<embedded_sdmmc::Error<T>> for FSError {
    fn from(value: embedded_sdmmc::Error<T>) -> Self {
        use embedded_sdmmc::Error::*;
        match value {
            DeviceError(e) => Self::DeviceError(alloc::format!("{e:?}")),
            FormatError(e) => Self::FormatError(e),
            NoSuchVolume => Self::NoSuchVolume,
            FilenameError(e) => Self::FilenameError(e),
            TooManyOpenVolumes => Self::TooManyOpenVolumes,
            TooManyOpenDirs => Self::TooManyOpenDirs,
            TooManyOpenFiles => Self::TooManyOpenFiles,
            BadHandle => Self::BadHandle,
            NotFound => Self::NotFound,
            FileAlreadyOpen => Self::FileAlreadyOpen,
            DirAlreadyOpen => Self::DirAlreadyOpen,
            OpenedDirAsFile => Self::OpenedDirAsFile,
            OpenedFileAsDir => Self::OpenedFileAsDir,
            DeleteDirAsFile => Self::DeleteDirAsFile,
            VolumeStillInUse => Self::VolumeStillInUse,
            VolumeAlreadyOpen => Self::VolumeAlreadyOpen,
            Unsupported => Self::Unsupported,
            EndOfFile => Self::EndOfFile,
            BadCluster => Self::BadCluster,
            ConversionError => Self::ConversionError,
            NotEnoughSpace => Self::NotEnoughSpace,
            AllocationError => Self::AllocationError,
            UnterminatedFatChain => Self::UnterminatedFatChain,
            ReadOnly => Self::ReadOnly,
            FileAlreadyExists => Self::FileAlreadyExists,
            BadBlockSize(size) => Self::BadBlockSize(size),
            InvalidOffset => Self::InvalidOffset,
            DiskFull => Self::DiskFull,
            DirAlreadyExists => Self::DirAlreadyExists,
            LockError => Self::Deadlock,
        }
    }
}

#[cfg(not(target_os = "none"))]
impl From<std::io::Error> for FSError {
    fn from(value: std::io::Error) -> Self {
        value.kind().into()
    }
}

#[cfg(not(target_os = "none"))]
impl From<std::io::ErrorKind> for FSError {
    fn from(value: std::io::ErrorKind) -> Self {
        use std::io::ErrorKind::*;
        match value {
            NotFound => Self::NotFound,
            PermissionDenied => Self::PermissionDenied,
            ConnectionRefused => Self::ConnectionRefused,
            ConnectionReset => Self::ConnectionReset,
            ConnectionAborted => Self::ConnectionAborted,
            NotConnected => Self::NotConnected,
            AddrInUse => Self::AddrInUse,
            AddrNotAvailable => Self::AddrNotAvailable,
            BrokenPipe => Self::BrokenPipe,
            AlreadyExists => Self::FileAlreadyExists,
            WouldBlock => Self::Other,
            InvalidInput => Self::InvalidInput,
            InvalidData => Self::InvalidData,
            TimedOut => Self::TimedOut,
            WriteZero => Self::WriteZero,
            Interrupted => Self::Interrupted,
            Unsupported => Self::Unsupported,
            UnexpectedEof => Self::EndOfFile,
            OutOfMemory => Self::DiskFull,
            Other => Self::Other,
            _ => Self::Other,
        }
    }
}

impl From<embedded_io::ErrorKind> for FSError {
    fn from(value: embedded_io::ErrorKind) -> Self {
        use embedded_io::ErrorKind::*;
        match value {
            Other => Self::Other,
            NotFound => Self::NotFound,
            PermissionDenied => Self::PermissionDenied,
            ConnectionRefused => Self::ConnectionRefused,
            ConnectionReset => Self::ConnectionReset,
            ConnectionAborted => Self::ConnectionAborted,
            NotConnected => Self::NotConnected,
            AddrInUse => Self::AddrInUse,
            AddrNotAvailable => Self::AddrNotAvailable,
            BrokenPipe => Self::BrokenPipe,
            AlreadyExists => Self::FileAlreadyExists,
            InvalidInput => Self::InvalidInput,
            InvalidData => Self::InvalidData,
            TimedOut => Self::TimedOut,
            Interrupted => Self::Interrupted,
            Unsupported => Self::Unsupported,
            OutOfMemory => Self::DiskFull,
            WriteZero => Self::WriteZero,
            _ => Self::Other,
        }
    }
}

impl fmt::Display for FSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use FSError::*;
        match self {
            DeviceError(e) => write!(f, "device error: {e}"),
            FormatError(e) => write!(f, "format error: {e}"),
            NoSuchVolume => write!(f, "no such volume"),
            FilenameError(e) => write!(f, "filename error: {e:?}"),
            TooManyOpenVolumes => write!(f, "too many open volumes"),
            TooManyOpenDirs => write!(f, "too many open dirs"),
            TooManyOpenFiles => write!(f, "too many open files"),
            BadHandle => write!(f, "bad handle"),
            NotFound => write!(f, "not found"),
            FileAlreadyOpen => write!(f, "file already open"),
            DirAlreadyOpen => write!(f, "dir already open"),
            OpenedDirAsFile => write!(f, "opened dir as file"),
            OpenedFileAsDir => write!(f, "opened file as dir"),
            DeleteDirAsFile => write!(f, "delete dir as file"),
            VolumeStillInUse => write!(f, "volume still in use"),
            VolumeAlreadyOpen => write!(f, "volume already open"),
            Unsupported => write!(f, "unsupported"),
            EndOfFile => write!(f, "end of file"),
            BadCluster => write!(f, "bad cluster"),
            ConversionError => write!(f, "conversion error"),
            NotEnoughSpace => write!(f, "not enough space"),
            AllocationError => write!(f, "allocation error"),
            UnterminatedFatChain => write!(f, "unterminated fat chain"),
            ReadOnly => write!(f, "read only"),
            FileAlreadyExists => write!(f, "file already exists"),
            BadBlockSize(_) => write!(f, "bad block size"),
            InvalidOffset => write!(f, "invalid offset"),
            DiskFull => write!(f, "disk full"),
            DirAlreadyExists => write!(f, "dir already exists"),
            PermissionDenied => write!(f, "permission denied"),
            ConnectionRefused => write!(f, "connection refused"),
            ConnectionReset => write!(f, "connection reset"),
            ConnectionAborted => write!(f, "connection aborted"),
            NotConnected => write!(f, "not connected"),
            AddrInUse => write!(f, "addr in use"),
            AddrNotAvailable => write!(f, "addr not available"),
            BrokenPipe => write!(f, "broken pipe"),
            InvalidInput => write!(f, "invalid input"),
            InvalidData => write!(f, "invalid data"),
            TimedOut => write!(f, "timed out"),
            Interrupted => write!(f, "interrupted"),
            WriteZero => write!(f, "write zero"),
            Deadlock => write!(f, "deadlock"),
            Other => write!(f, "other"),
        }
    }
}

pub enum NetworkError {
    NotInitialized,
    AlreadyInitialized,
    UnknownPeer,
    CannotBind,
    PeerListFull,
    RecvError,
    SendError,
    NetThreadDeallocated,
    OutMessageTooBig,
    Other(u32),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use NetworkError::*;
        match self {
            NotInitialized => write!(f, "cannot send messages with Wi-Fi turned off"),
            AlreadyInitialized => write!(f, "tried to initialize networking twice"),
            UnknownPeer => write!(f, "cannot send messages to disconnected device"),
            CannotBind => write!(f, "cannot find free address for networking"),
            PeerListFull => write!(f, "cannot connect more devices"),
            RecvError => write!(f, "cannot fetch network message"),
            SendError => write!(f, "cannot send network message"),
            NetThreadDeallocated => write!(f, "thread handling networking is already deallocated"),
            OutMessageTooBig => write!(f, "outgoing message is too big"),
            Other(n) => write!(f, "network error #{n}"),
        }
    }
}
