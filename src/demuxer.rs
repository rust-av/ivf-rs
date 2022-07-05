//!
//! Implement the demuxer trait from av-format and expose all the correct
//! abstraction to handle them. Refer to the `Demuxer` trait for more info.
//!
//! Internally the parsing is implement with the `nom` parser
//!

use crate::common::Codec;
use av_bitstream::byteread::*;
use av_data::packet::Packet;
use av_data::params::{CodecParams, MediaKind, VideoInfo};
use av_data::rational::Rational64;
use av_data::timeinfo::TimeInfo;
use av_format::buffer::Buffered;
use av_format::common::GlobalInfo;
use av_format::demuxer::{Demuxer, Event};
use av_format::demuxer::{Descr, Descriptor};
use av_format::error::*;
use av_format::stream::Stream;
use nom::bytes::streaming::tag;
use nom::bytes::streaming::take;
use nom::error::ErrorKind;
use nom::sequence::tuple;
use nom::{Err, IResult, Needed, Offset};
use std::collections::VecDeque;
use std::io::SeekFrom;

#[derive(Default)]
pub struct IvfDemuxer {
    header: Option<IvfHeader>,
    queue: VecDeque<Event>,
}

#[derive(Clone, Debug)]
pub struct IvfHeader {
    #[allow(dead_code)]
    version: u16,
    width: u16,
    height: u16,
    rate: u32,
    #[allow(dead_code)]
    scale: u32,
    codec: Codec,
    nframe: u32,
}

#[derive(Debug, PartialEq)]
pub struct IvfFrame {
    size: u32,
    pos: u64,
    data: Vec<u8>,
}

impl IvfDemuxer {
    pub fn new() -> IvfDemuxer {
        Default::default()
    }
}

impl Demuxer for IvfDemuxer {
    fn read_headers(&mut self, buf: &mut dyn Buffered, info: &mut GlobalInfo) -> Result<SeekFrom> {
        match ivf_header(buf.data()) {
            Ok((input, header)) => {
                debug!("found header: {:?}", header);
                let st = Stream {
                    id: 0,
                    index: 0,
                    params: CodecParams {
                        extradata: None,
                        bit_rate: header.rate as usize,
                        delay: 0,
                        convergence_window: 0,
                        codec_id: Some(header.codec.into()),
                        kind: Some(MediaKind::Video(VideoInfo {
                            width: header.width as usize,
                            height: header.height as usize,
                            format: None,
                        })),
                    },
                    start: None,
                    duration: Some(header.nframe as u64),
                    timebase: Rational64::new(1, 1000 * 1000 * 1000),
                    user_private: None,
                };
                self.header = Some(header);
                info.add_stream(st);
                Ok(SeekFrom::Current(buf.data().offset(input) as i64))
            }
            Err(e) => {
                error!("error reading headers: {:?}", e);
                Err(Error::InvalidData)
            }
        }
    }

    fn read_event(&mut self, buf: &mut dyn Buffered) -> Result<(SeekFrom, Event)> {
        if let Some(event) = self.queue.pop_front() {
            Ok((SeekFrom::Current(0), event))
        } else {
            // check for EOF
            if buf.data().is_empty() {
                return Ok((SeekFrom::Current(0), Event::Eof));
            }

            // feed with more stuff
            match ivf_frame(buf.data()) {
                Ok((input, frame)) => {
                    debug!("found frame with size: {}\tpos: {}", frame.size, frame.pos);

                    let pkt = Packet {
                        data: frame.data,
                        pos: Some(frame.pos as usize),
                        stream_index: 0,
                        t: TimeInfo::default(),
                        is_key: false,
                        is_corrupted: false,
                    };

                    Ok((
                        SeekFrom::Current(buf.data().offset(input) as i64),
                        Event::NewPacket(pkt),
                    ))
                }
                Err(Err::Incomplete(needed)) => {
                    let sz = match needed {
                        Needed::Size(size) => buf.data().len() + size.get(),
                        Needed::Unknown => 1024,
                    };
                    Err(Error::MoreDataNeeded(sz))
                }
                Err(e) => {
                    error!("error reading frame: {:#?}", e);
                    Err(Error::InvalidData)
                }
            }
        }
    }
}

/// take data ownership
pub fn parse_binary_data(input: &[u8], size: u64) -> IResult<&[u8], Vec<u8>> {
    take(size as usize)(input).map(|(input, s)| (input, s.to_vec()))
}

/// u16 nom help function that maps to av-bitstream
pub fn parse_u16(input: &[u8]) -> IResult<&[u8], u16> {
    Ok((&input[2..], get_u16l(&input[0..2])))
}

/// u32 nom help function that maps to av-bitstream
pub fn parse_u32(input: &[u8]) -> IResult<&[u8], u32> {
    Ok((&input[4..], get_u32l(&input[0..4])))
}

/// u64 nom help function that maps to av-bitstream
pub fn parse_u64(input: &[u8]) -> IResult<&[u8], u64> {
    Ok((&input[8..], get_u64l(&input[0..8])))
}

/// use ErrorKind::Tag that could be a bit confusing
pub fn parse_codec(input: &[u8]) -> IResult<&[u8], Codec> {
    let codec = match &input[0..4] {
        b"VP80" => Codec::VP8,
        b"VP90" => Codec::VP9,
        b"AV01" => Codec::AV1,
        _ => {
            return Err(nom::Err::Error(error_position!(
                &input[0..4],
                ErrorKind::Tag
            )))
        }
    };

    Ok((&input[4..], codec))
}

// TODO: validate values
pub fn ivf_header(input: &[u8]) -> IResult<&[u8], IvfHeader> {
    tuple((
        tag("DKIF"),
        parse_u16,
        parse_u16,
        parse_codec,
        parse_u16,
        parse_u16,
        parse_u32,
        parse_u32,
        parse_u32,
        take(4usize),
    ))(input)
    .map(
        |(input, (_tag, version, _length, codec, width, height, rate, scale, nframe, _))| {
            (
                input,
                IvfHeader {
                    version,
                    width,
                    height,
                    rate,
                    scale,
                    codec,
                    nframe,
                },
            )
        },
    )
}

// (frame_size > 256 * 1024 * 1024)
pub fn ivf_frame(input: &[u8]) -> IResult<&[u8], IvfFrame> {
    tuple((parse_u32, parse_u64))(input)
        .and_then(|(input, (size, pos))| {
            let (input, data) = take(size)(input)?;
            Ok((input, (size, pos, data)))
        })
        .map(|(input, (size, pos, data))| {
            (
                input,
                IvfFrame {
                    size,
                    pos,
                    data: data.to_owned(),
                },
            )
        })
}

struct Des {
    d: Descr,
}

impl Descriptor for Des {
    type OutputDemuxer = IvfDemuxer;

    fn create(&self) -> Self::OutputDemuxer {
        IvfDemuxer::new()
    }
    fn describe(&self) -> &Descr {
        &self.d
    }
    fn probe(&self, data: &[u8]) -> u8 {
        match ivf_header(&data[..=32]) {
            Ok(_) => 32,
            _ => 0,
        }
    }
}

/// used by av context
pub const IVF_DESC: &dyn Descriptor<OutputDemuxer = IvfDemuxer> = &Des {
    d: Descr {
        name: "ivf-rs",
        demuxer: "ivf",
        description: "Nom-based Ivf demuxer",
        extensions: &["vp8", "vp9", "av1"],
        mime: &[],
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use av_format::buffer::AccReader;
    use av_format::demuxer::Context;
    use std::io::Cursor;

    const IVF: &[u8] = include_bytes!("../assets/single_stream_av1.ivf");

    #[test]
    fn parse_headers() {
        let _ = pretty_env_logger::try_init();

        let descriptor = IVF_DESC.create();
        let cursor = Cursor::new(IVF);
        let acc = AccReader::new(cursor);

        let mut demuxer = Context::new(descriptor, acc);

        match demuxer.read_headers() {
            Ok(_) => debug!("Headers read correctly"),
            Err(e) => {
                panic!("error: {:?}", e);
            }
        }

        trace!("global info: {:#?}", demuxer.info);
    }

    #[test]
    fn demux() {
        let _ = pretty_env_logger::try_init();
        let descriptor = IVF_DESC.create();
        let cursor = Cursor::new(IVF);
        let acc = AccReader::new(cursor);
        let mut demuxer = Context::new(descriptor, acc);
        demuxer.read_headers().unwrap();

        trace!("global info: {:#?}", demuxer.info);

        loop {
            match demuxer.read_event() {
                Ok(event) => match event {
                    Event::MoreDataNeeded(sz) => panic!("we needed more data: {} bytes", sz),
                    Event::NewStream(s) => panic!("new stream :{:?}", s),
                    Event::NewPacket(packet) => {
                        debug!("received packet with pos: {:?}", packet.pos);
                    }
                    Event::Continue => continue,
                    Event::Eof => {
                        debug!("EOF!");
                        break;
                    }
                    _ => unimplemented!(),
                },
                Err(e) => {
                    panic!("error: {:?}", e);
                }
            }
        }
    }
}
