use av_bitstream::byteread::*;
use av_data::packet::Packet;
use av_data::timeinfo::TimeInfo;
use av_format::buffer::Buffered;
use av_format::common::GlobalInfo;
use av_format::demuxer::{Demuxer, Event};
use av_format::demuxer::{Descr, Descriptor};
use av_format::error::*;
use nom::{IResult, Offset, ErrorKind};
use std::collections::VecDeque;
use std::io::SeekFrom;
use common::Codec;

#[derive(Default)]
pub struct IvfDemuxer {
    header: Option<IvfHeader>,
    pub queue: VecDeque<Event>,
}

#[derive(Debug)]
pub struct IvfHeader {
    version: u16,
    width: u16,
    height: u16,
    rate: u32,
    scale: u32,
    codec: Codec
}

#[derive(Debug)]
pub struct IvfFrame {
    pos: u64,
    size: u32,
    data: Vec<u8>,
}

impl IvfDemuxer {
    pub fn new() -> IvfDemuxer {
        Default::default()
    }
}

impl Demuxer for IvfDemuxer {
    fn read_headers(&mut self, buf: &Box<Buffered>, _info: &mut GlobalInfo) -> Result<SeekFrom> {
        match ivf_header(buf.data()) {
            Ok((input, header)) => {
                debug!("found header: {:?}", header);
                self.header = Some(header);
                Ok(SeekFrom::Current(buf.data().offset(input) as i64))
            }
            Err(e) => {
                error!("error reading headers: {:?}", e);
                Err(Error::InvalidData)
            }
        }
    }

    fn read_event(&mut self, buf: &Box<Buffered>) -> Result<(SeekFrom, Event)> {
        if let Some(event) = self.queue.pop_front() {
            Ok((SeekFrom::Current(0), event))
        } else {
            // check for EOF
            if buf.data().len() == 0 {
                return Ok((SeekFrom::Current(0), Event::MoreDataNeeded(0)));
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

                    return Ok((
                        SeekFrom::Current(buf.data().offset(input) as i64),
                        Event::NewPacket(pkt),
                    ));
                }
                Err(e) => {
                    error!("error reading frame: {:#?}", e);
                    return Err(Error::InvalidData);
                }
            }
        }
    }
}

pub fn parse_binary_data(input: &[u8], size: u64) -> IResult<&[u8], Vec<u8>> {
    do_parse!(input, s: take_s!(size as usize) >> (s.to_owned()))
}

pub fn parse_u16(input: &[u8]) -> IResult<&[u8], u16> {
    Ok((&input[2..], get_u16l(&input[0..2])))
}

pub fn parse_u32(input: &[u8]) -> IResult<&[u8], u32> {
    Ok((&input[4..], get_u32l(&input[0..4])))
}

pub fn parse_u64(input: &[u8]) -> IResult<&[u8], u64> {
    Ok((&input[8..], get_u64l(&input[0..8])))
}

pub fn parse_codec(input: &[u8]) -> IResult<&[u8], Codec> {
    let codec = match &input[0..4] {
        b"VP80" => Codec::VP8,
        b"VP90" => Codec::VP9,
        b"AV1" => Codec::AV1,
        _ => return Err(nom::Err::Error(error_position!(&input[0..4], ErrorKind::Tag)))
    };

    Ok((&input[4..], codec))
}

// TODO: validate values
named!(ivf_header<&[u8], IvfHeader>,
       do_parse!(
           tag!("DKIF")
           >> version: parse_u16
           >> _length: parse_u16
           >> codec: parse_codec
           >> width: parse_u16
           >> height: parse_u16
           >> rate: parse_u32
           >> scale: parse_u32
           >> take!(8)
           >> (IvfHeader {version, width, height, rate, scale, codec})
       )
);

// (frame_size > 256 * 1024 * 1024)
named!(ivf_frame<&[u8], IvfFrame>,
       do_parse!(
           size: parse_u32
           >> pos: parse_u64
           >> data: apply!(parse_binary_data, size as u64)
           >> (IvfFrame { size, pos, data })
           )
      );

struct Des {
    d: Descr,
}

impl Descriptor for Des {
    fn create(&self) -> Box<Demuxer> {
        Box::new(IvfDemuxer::new())
    }
    fn describe<'a>(&'a self) -> &'a Descr {
        &self.d
    }
    fn probe(&self, data: &[u8]) -> u8 {
        match ivf_header(&data[..=32]) {
            Ok(_) => 32,
            _ => 0,
        }
    }
}

pub const IVF_DESC: &Descriptor = &Des {
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

    const IVF: &'static [u8] = include_bytes!("../assets/vp80-00-comprehensive-001.ivf");

    #[test]
    fn demux() {
        let _ = pretty_env_logger::try_init();
        let d = IVF_DESC.create();
        let c = Cursor::new(IVF);
        let acc = AccReader::with_capacity(20000, c);
        let input = Box::new(acc);
        let mut demuxer = Context::new(d, input);
        demuxer.read_headers().unwrap();

        trace!("global info: {:#?}", demuxer.info);

        loop {
            match demuxer.read_event() {
                Ok(event) => match event {
                    Event::MoreDataNeeded(sz) => panic!("we needed more data: {} bytes", sz),
                    Event::NewStream(s) => panic!("new stream :{:?}", s),
                    Event::NewPacket(packet) => {
                        trace!("received packet with pos: {:?}", packet.pos);
                    }
                    Event::Eof => {
                        trace!("EOF!");
                        break;
                    }
                },
                Err(e) => {
                    trace!("error: {:?}", e);
                    break;
                }
            }
        }
    }
}
