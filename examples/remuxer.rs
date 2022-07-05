use std::fs::File;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use log::{debug, trace};

use av_format::buffer::AccReader;
use av_format::demuxer::Context as DemuxerContext;
use av_format::demuxer::Event;
use av_format::muxer::{Context as MuxerContext, Writer};

use av_ivf::demuxer::*;
use av_ivf::muxer::*;

#[derive(Parser, Debug)]
#[clap(name = "ivf remux")]
/// Simple Audio Video Encoding tool
struct Opts {
    /// Input file
    #[clap(short = 'i', value_parser)]
    input: PathBuf,
    /// Output file
    #[clap(short = 'o', value_parser)]
    output: PathBuf,
}

fn main() {
    let _ = pretty_env_logger::try_init();
    let opts = Opts::parse();

    let input = std::fs::File::open(opts.input).unwrap();
    let acc = AccReader::new(input);
    let mut demuxer = DemuxerContext::new(IvfDemuxer::new(), acc);

    demuxer.read_headers().unwrap();
    trace!("global info: {:#?}", demuxer.info);

    let mut output = File::create(opts.output).unwrap();

    let mut muxer = MuxerContext::new(
        IvfMuxer::new(),
        Writer::from_seekable(Cursor::new(Vec::new())),
    );

    muxer.set_global_info(demuxer.info.clone()).unwrap();
    muxer.configure().unwrap();
    muxer.write_header().unwrap();

    loop {
        match demuxer.read_event() {
            Ok(event) => match event {
                Event::MoreDataNeeded(sz) => panic!("we needed more data: {} bytes", sz),
                Event::NewStream(s) => panic!("new stream :{:?}", s),
                Event::NewPacket(packet) => {
                    debug!("received packet with pos: {:?}", packet.pos);
                    muxer.write_packet(Arc::new(packet)).unwrap();
                }
                Event::Continue => continue,
                Event::Eof => {
                    muxer.write_trailer().unwrap();
                    debug!("EOF!");
                    break;
                }
                _ => unimplemented!(),
            },
            Err(e) => {
                debug!("error: {:?}", e);
                break;
            }
        }
    }

    output
        .write_all(&muxer.writer().seekable_object().unwrap().into_inner())
        .unwrap();
}
