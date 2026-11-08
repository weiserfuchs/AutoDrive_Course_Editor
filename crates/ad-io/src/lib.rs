//! I/O layer: config XML, vehicles and routes, images and settings.
//!
//! Depends on `ad-core` and never on the UI.
mod atomic;
pub mod config_xml;
pub mod dds;
pub mod map_image;
pub mod mod_zip;
pub mod settings;
pub mod vehicles;
