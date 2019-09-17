#[derive(Debug)]
pub enum Codec {
    VP8,
    VP9,
    AV1,
}

impl Default for Codec {
    fn default() -> Codec {
        Codec::VP8
    }
}
