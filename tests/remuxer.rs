#[macro_use]
extern crate log;
extern crate av_format;
extern crate av_ivf;

use av_format::buffer::AccReader;
use av_format::demuxer::Context as DemuxerContext;
use av_format::demuxer::Event;
use av_format::muxer::{Context as MuxerContext, Writer};
use av_ivf::demuxer::*;
use av_ivf::muxer::*;
use std::fs::File;
use std::io::{Cursor, Write};
use std::sync::Arc;

const IVF: &str = "assets/single_stream_av1.ivf";
const IVF_OUTPUT: &str = "assets/out_av1.ivf";

fn demux_mux() {
    let input_file = File::open(IVF).unwrap();
    let mut demuxer = DemuxerContext::new(IvfDemuxer::new(), AccReader::new(input_file));

    demuxer.read_headers().unwrap();

    let mut output_file = File::create(IVF_OUTPUT).unwrap();
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
                error!("error: {:?}", e);
                break;
            }
        }
    }

    output_file
        .write_all(&muxer.writer().seekable_object().unwrap().into_inner())
        .unwrap();
}

fn check_mux() {
    let demuxer_original_file = File::open(IVF).unwrap();
    let mut demuxer_original =
        DemuxerContext::new(IvfDemuxer::new(), AccReader::new(demuxer_original_file));

    let demuxer_file = File::open(IVF_OUTPUT).unwrap();
    let mut demuxer = DemuxerContext::new(IvfDemuxer::new(), AccReader::new(demuxer_file));

    demuxer_original.read_headers().unwrap();
    demuxer.read_headers().unwrap();

    loop {
        match (demuxer_original.read_event(), demuxer.read_event()) {
            (Ok(event_original), Ok(event)) => match (event_original, event) {
                (Event::MoreDataNeeded(sz), Event::MoreDataNeeded(sz1)) => {
                    assert_eq!(sz, sz1);
                }
                (Event::NewStream(s), Event::NewStream(s1)) => {
                    assert_eq!(s.params, s1.params);
                    assert_eq!(s.duration, s1.duration);
                }
                (Event::NewPacket(packet), Event::NewPacket(packet1)) => {
                    assert_eq!(packet.data, packet1.data);
                    assert_eq!(packet.pos, packet1.pos);
                }
                (Event::Continue, Event::Continue) => continue,
                (Event::Eof, Event::Eof) => {
                    debug!("EOF!");
                    break;
                }
                (_, _) => panic!("Different events for demuxers that act on the same content"),
            },
            (Err(e_original), Err(e)) => {
                assert_eq!(format!("{:?}", e_original), format!("{:?}", e));
                break;
            }
            (_, _) => {
                panic!("The two demuxers do not get the same output");
            }
        }
    }
}

#[test]
fn remuxer() {
    let _ = pretty_env_logger::try_init();

    // Demux ivf file and remux it
    demux_mux();

    // Check if the muxed ivf file is equal to the original
    check_mux();
}
