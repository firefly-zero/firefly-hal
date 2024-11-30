#![cfg_attr(target_os = "none", no_std)]

#[cfg(not(target_os = "none"))]
mod gamepad;

mod shared;

#[cfg_attr(target_family = "wasm", path = "web.rs")]
#[cfg_attr(not(target_os = "none"), path = "hosted.rs")]
#[cfg_attr(target_os = "none", path = "embedded.rs")]
mod device;

#[cfg(not(target_os = "none"))]
pub use device::DeviceConfig;

pub use device::{DeviceImpl, NetworkImpl, SerialImpl};
pub use shared::*;
