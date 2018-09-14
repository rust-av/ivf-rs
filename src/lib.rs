#![feature(rust_2018_preview)]

//! This is a simple implementation of muxer/demuxer for Ivf file format.
//!
//! ## Example
//!
//! \[wip\] look at test in mux/demux
//!
//! ## About Ivf
//!
//! Refer to the [specification](https://wiki.multimedia.cx/index.php/IVF).
//!
//! ## Relies on rust-av
//!
//! This projects relies on [rust-av](https://github.com/rust-av/rust-av) toolkit
//!

extern crate av_bitstream;
extern crate av_data;
extern crate av_format;
#[macro_use]
extern crate nom;
#[macro_use]
extern crate log;

#[cfg(test)]
extern crate tempfile;
#[cfg(test)]
extern crate pretty_env_logger;

pub mod demux;
pub mod mux;
mod common;
