#![cfg_attr(target_os = "none", no_std)]

mod shared;

#[cfg_attr(not(target_os = "none"), path = "hosted.rs")]
#[cfg_attr(target_os = "none", path = "embedded.rs")]
mod device;

pub use device::get_device;
pub use shared::Device;
