//!
//! Implement the muxer trait from av-format and expose all the correct
//! abstraction to handle them. Refer to the `Muxer` trait for more info.
//!
//!

use std::io::Write;
use std::sync::Arc;

use log::{debug, trace};

use av_bitstream::bytewrite::*;
use av_data::packet::Packet;
use av_data::params::MediaKind;
use av_data::rational::Rational32;
use av_data::value::Value;
use av_format::common::GlobalInfo;
use av_format::error::*;
pub use av_format::muxer::Muxer;
pub use av_format::muxer::{Context, Writer};

use crate::common::Codec;

#[derive(Debug)]
pub struct IvfMuxer {
    version: u16,
    width: u16,
    height: u16,
    frame_rate: Rational32,
    scale: u32,
    codec: Codec,
    duration: u32,
    info: Option<GlobalInfo>,
}

impl Default for IvfMuxer {
    fn default() -> Self {
        IvfMuxer {
            frame_rate: Rational32::new(30, 1),
            version: Default::default(),
            width: Default::default(),
            height: Default::default(),
            scale: Default::default(),
            codec: Default::default(),
            duration: Default::default(),
            info: Default::default(),
        }
    }
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
                self.frame_rate = info
                    .timebase
                    .map(|tb| Rational32::new(*tb.denom() as i32, *tb.numer() as i32))
                    .unwrap_or_else(|| Rational32::new(30, 1));
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

    fn write_header<W: Write>(&mut self, buf: &mut Writer<W>) -> Result<()> {
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
        put_u32l(&mut tmp_buf[4..8], *self.frame_rate.numer() as u32);
        put_u32l(&mut tmp_buf[8..12], *self.frame_rate.denom() as u32);
        put_u32l(&mut tmp_buf[12..16], self.duration);
        put_u32l(&mut tmp_buf[16..20], 0);
        buf.write_all(&tmp_buf)?;

        Ok(())
    }

    fn write_packet<W: Write>(&mut self, buf: &mut Writer<W>, pkt: Arc<Packet>) -> Result<()> {
        trace!("Write packet: {:?}", pkt.pos);

        let mut frame_header = [0; 12];

        put_u32l(&mut frame_header[0..4], pkt.data.len() as u32);
        put_u64l(&mut frame_header[4..12], pkt.pos.unwrap_or_default() as u64);

        buf.write_all(&frame_header)?;
        buf.write_all(&pkt.data)?;

        Ok(())
    }

    fn write_trailer<W: Write>(&mut self, buf: &mut Writer<W>) -> Result<()> {
        buf.flush()?;
        Ok(())
    }

    fn set_global_info(&mut self, info: GlobalInfo) -> Result<()> {
        self.info = Some(info);
        Ok(())
    }

    fn set_option<'a>(&mut self, key: &str, val: Value<'a>) -> Result<()> {
        match key {
            "frame_rate" => {
                self.frame_rate = get_val_rational(val)?;
            }
            "width" => {
                self.width = get_val_int(val)? as u16;
            }
            "height" => {
                self.height = get_val_int(val)? as u16;
            }
            "scale" => {
                self.scale = get_val_int(val)? as u32;
            }
            "duration" => {
                self.duration = get_val_int(val)? as u32;
            }
            _ => {
                return Err(av_format::error::Error::InvalidData);
            }
        };
        Ok(())
    }
}

fn get_val_rational(val: Value<'_>) -> Result<Rational32> {
    match val {
        Value::I64(val) => Ok(Rational32::new(val as i32, 1)),
        Value::U64(val) => Ok(Rational32::new(val as i32, 1)),
        Value::Pair(numer, denom) => Ok(Rational32::new(numer as i32, denom as i32)),
        _ => Err(av_format::error::Error::InvalidData),
    }
}

fn get_val_int(val: Value<'_>) -> Result<i64> {
    match val {
        Value::I64(val) => Ok(val),
        Value::U64(val) => Ok(val as i64),
        _ => Err(av_format::error::Error::InvalidData),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use av_format::common::GlobalInfo;
    use av_format::muxer::{Context, Writer};

    use super::*;

    #[test]
    fn mux() {
        let _ = pretty_env_logger::try_init();

        let info = GlobalInfo {
            duration: None,
            timebase: None,
            streams: Vec::new(),
        };

        let mut muxer = Context::new(IvfMuxer::new(), Writer::new(Cursor::new(Vec::new())));

        muxer.set_global_info(info).unwrap();
        muxer.configure().unwrap();
        muxer.write_header().unwrap();

        tempfile::tempfile()
            .unwrap()
            .write_all(muxer.writer().as_ref().0.get_ref())
            .unwrap();
    }
}
