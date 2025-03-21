#![cfg_attr(target_os = "none", no_std)]
#![allow(clippy::new_without_default)]

extern crate alloc;

#[cfg(not(target_os = "none"))]
mod gamepad;

mod errors;
mod shared;

#[cfg_attr(target_family = "wasm", path = "web.rs")]
#[cfg_attr(not(target_os = "none"), path = "hosted.rs")]
#[cfg_attr(target_os = "none", path = "embedded.rs")]
mod device;

#[cfg(not(target_os = "none"))]
pub use device::DeviceConfig;

pub use device::{Addr, DeviceImpl, NetworkImpl, SerialImpl};
pub use errors::*;
pub use shared::*;
