//!
//! Implement the muxer trait from av-format and expose all the correct
//! abstraction to handle them. Refer to the `Muxer` trait for more info.
//!
//!

use crate::common::Codec;
use av_data::packet::Packet;
use av_data::params::MediaKind;
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
    duration: u32,
    info: Option<GlobalInfo>,
}

impl IvfMuxer {
    pub fn new() -> IvfMuxer {
        IvfMuxer::default()
    }
}

/// This should be called if IvfMuxer::info is set
impl Muxer for IvfMuxer {
    fn configure(&mut self) -> Result<()> {
        match self.info.as_ref() {
            Some(info) if !info.streams.is_empty() => {
                self.duration = info.streams[0].duration.unwrap_or_default() as u32;
                let params = &info.streams[0].params;
                self.version = 0;
                if let Some(MediaKind::Video(video)) = &params.kind {
                    self.width = video.width as u16;
                    self.height = video.height as u16;
                };
                // TODO: parse scale
                self.rate = params.bit_rate as u32;
                self.scale = 1;
                self.codec = match params.codec_id.as_deref() {
                    Some("av1") => Codec::AV1,
                    Some("vp8") => Codec::VP8,
                    Some("vp9") => Codec::VP9,
                    _ => Codec::default(),
                };

                debug!("Configuration changes {:?}", self);

                Ok(())
            }

            _ => {
                debug!("No configuration changes {:?}", self);
                Ok(())
            }
        }
    }

    fn write_header(&mut self, buf: &mut dyn Write) -> Result<()> {
        debug!("Write muxer header: {:?}", self);

        let codec = match self.codec {
            Codec::VP8 => b"VP80",
            Codec::VP9 => b"VP90",
            Codec::AV1 => b"AV01",
        };

        let mut tmp_buf = [0u8; 20];
        buf.write_all(b"DKIF")?;
        put_u16l(&mut tmp_buf[0..2], self.version);
        put_u16l(&mut tmp_buf[2..4], 32);
        buf.write_all(&tmp_buf[..4])?;
        buf.write_all(codec)?;
        put_u16l(&mut tmp_buf[0..2], self.width);
        put_u16l(&mut tmp_buf[2..4], self.height);
        put_u32l(&mut tmp_buf[4..8], self.rate);
        put_u32l(&mut tmp_buf[8..12], self.scale);
        put_u32l(&mut tmp_buf[12..16], self.duration);
        put_u32l(&mut tmp_buf[16..20], 0);
        buf.write_all(&tmp_buf)?;

        Ok(())
    }

    fn write_packet(&mut self, buf: &mut dyn Write, pkt: Arc<Packet>) -> Result<()> {
        trace!("Write packet: {:?}", pkt.pos);

        let mut frame_header = [0; 12];

        put_u32l(&mut frame_header[0..4], pkt.data.len() as u32);
        put_u64l(&mut frame_header[4..12], pkt.pos.unwrap_or_default() as u64);

        buf.write_all(&frame_header)?;
        buf.write_all(&pkt.data)?;

        Ok(())
    }

    fn write_trailer(&mut self, _buf: &mut dyn Write) -> Result<()> {
        Ok(())
    }

    fn set_global_info(&mut self, info: GlobalInfo) -> Result<()> {
        self.info = Some(info);
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

        muxer.set_global_info(info).unwrap();
        muxer.configure().unwrap();
        muxer.write_header().unwrap();
    }
}
