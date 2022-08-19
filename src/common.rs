pub use av_format::common::GlobalInfo;

#[derive(Copy, Clone, Debug)]
pub(crate) enum Codec {
    VP8,
    VP9,
    AV1,
}

impl Default for Codec {
    fn default() -> Codec {
        Codec::VP8
    }
}

impl From<Codec> for String {
    fn from(other: Codec) -> String {
        match other {
            Codec::VP8 => String::from("vp8"),
            Codec::VP9 => String::from("vp9"),
            Codec::AV1 => String::from("av1"),
        }
    }
}
