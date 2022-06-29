#[macro_use]
extern crate log;
extern crate av_format;
extern crate av_ivf;
extern crate structopt;

use av_format::buffer::AccReader;
use av_format::demuxer::Context as DemuxerContext;
use av_format::demuxer::Event;
use av_format::muxer::{Context as MuxerContext, Writer};
use av_ivf::demuxer::*;
use av_ivf::muxer::*;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "ivf remux")]
/// Simple Audio Video Encoding tool
struct Opt {
    /// Input file
    #[structopt(short = "i", parse(from_os_str))]
    input: PathBuf,
    /// Output file
    #[structopt(short = "o", parse(from_os_str))]
    output: PathBuf,
}

fn main() {
    let _ = pretty_env_logger::try_init();
    let opt = Opt::from_args();

    let input = std::fs::File::open(opt.input).unwrap();

    let acc = AccReader::new(input);

    let mut demuxer = DemuxerContext::new(Box::new(IvfDemuxer::new()), Box::new(acc));

    demuxer.read_headers().unwrap();

    trace!("global info: {:#?}", demuxer.info);

    let output = File::create(opt.output).unwrap();

    let mux = Box::new(IvfMuxer::new());

    let mut muxer = MuxerContext::new(mux, Writer::Seekable(Box::new(output)));

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
}
