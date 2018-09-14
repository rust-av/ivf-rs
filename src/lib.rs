#![feature(rust_2018_preview)]

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
