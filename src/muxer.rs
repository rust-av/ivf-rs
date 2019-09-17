//!
//! Implement the muxer trait from av-format and expose all the correct
//! abstraction to handle them. Refer to the `Muxer` trait for more info.
//!
//!

use crate::common::Codec;
use av_data::packet::Packet;
use av_data::value::Value;
use av_format::common::GlobalInfo;
use av_format::error::*;
use av_format::muxer::Muxer;
use std::sync::Arc;

use av_bitstream::bytewrite::*;
use std::io::Write;

#[derive(Default, Debug)]
pub struct IvfMuxer {
    version: u16,
    width: u16,
    height: u16,
    rate: u32,
    scale: u32,
    codec: Codec,
}

impl IvfMuxer {
    pub fn new() -> IvfMuxer {
        Default::default()
    }
}

impl Muxer for IvfMuxer {
    fn configure(&mut self) -> Result<()> {
        Ok(())
    }

    fn write_header(&mut self, buf: &mut Vec<u8>) -> Result<()> {
        trace!("Write muxer header: {:?}", self);

        buf.resize(32, 0);

        let codec = match self.codec {
            Codec::VP8 => b"VP80",
            Codec::VP9 => b"VP80",
            Codec::AV1 => b"VP80",
        };

        (&mut buf[0..=3]).write_all(b"DKIF")?;
        put_u16l(&mut buf[4..=5], self.version);
        put_u16l(&mut buf[6..=7], 32);
        (&mut buf[8..=11]).write_all(codec)?;
        put_u16l(&mut buf[12..=13], self.width);
        put_u16l(&mut buf[14..=15], self.height);
        put_u32l(&mut buf[16..=19], self.rate);
        put_u32l(&mut buf[20..=23], self.scale);
        put_u64l(&mut buf[24..=31], 0);

        Ok(())
    }

    fn write_packet(&mut self, buf: &mut Vec<u8>, pkt: Arc<Packet>) -> Result<()> {
        let mut frame_header = [0; 12];
        put_u32l(&mut frame_header[0..=3], pkt.data.len() as u32);
        if let Some(pos) = pkt.pos {
            put_u64l(&mut frame_header[3..], pos as u64);
        }
        buf.extend(&pkt.data);
        Ok(())
    }

    fn write_trailer(&mut self, _buf: &mut Vec<u8>) -> Result<()> {
        Ok(())
    }

    fn set_global_info(&mut self, _info: GlobalInfo) -> Result<()> {
        Ok(())
    }

    fn set_option<'a>(&mut self, _key: &str, _val: Value<'a>) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use av_format::common::GlobalInfo;
    use av_format::muxer::Context;
    use std::fs::File;

    #[test]
    fn mux() {
        let _ = pretty_env_logger::try_init();

        let output: File = tempfile::tempfile().unwrap();

        let info = GlobalInfo {
            duration: None,
            timebase: None,
            streams: Vec::new(),
        };

        let mux = Box::new(IvfMuxer::new());

        let mut muxer = Context::new(mux, Box::new(output));

        muxer.configure().unwrap();
        muxer.set_global_info(info).unwrap();
        muxer.write_header().unwrap();
    }
}
