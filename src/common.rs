#[derive(Copy, Clone, Debug)]
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

impl Into<String> for Codec {
    fn into(self) -> String {
        match self {
            Codec::VP8 => String::from("vp8"),
            Codec::VP9 => String::from("vp9"),
            Codec::AV1 => String::from("av1"),
        }
    }
}
