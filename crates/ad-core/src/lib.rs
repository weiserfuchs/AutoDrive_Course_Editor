//! Core layer: the road network, commands, history and geometry.
//!
//! This crate performs no I/O and has no UI dependencies (AD-1).

pub mod command;
pub mod geometry;
pub mod history;
pub mod map_context;
pub mod network;
