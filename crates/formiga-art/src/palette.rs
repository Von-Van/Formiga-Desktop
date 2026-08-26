use crate::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub outline: Rgba,
    pub shadow: Rgba,
    pub coat: Rgba,
    pub highlight: Rgba,
    pub accent: Rgba,
    pub eye: Rgba,
}

const fn c(hex: u32) -> Rgba {
    Rgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

pub const PALETTES: [Palette; 12] = [
    Palette {
        outline: c(0x30243b),
        shadow: c(0x8d5b7b),
        coat: c(0xe98ab5),
        highlight: c(0xffc7dd),
        accent: c(0xffe081),
        eye: c(0x201827),
    },
    Palette {
        outline: c(0x24313b),
        shadow: c(0x4a7d8c),
        coat: c(0x75c9c8),
        highlight: c(0xc7f3e8),
        accent: c(0xffcf70),
        eye: c(0x172029),
    },
    Palette {
        outline: c(0x3a2d24),
        shadow: c(0xb56f45),
        coat: c(0xf2a65a),
        highlight: c(0xffd59a),
        accent: c(0x7dcfb6),
        eye: c(0x211914),
    },
    Palette {
        outline: c(0x28243d),
        shadow: c(0x6557a5),
        coat: c(0x9b8de3),
        highlight: c(0xd9d1ff),
        accent: c(0xff9f9f),
        eye: c(0x171426),
    },
    Palette {
        outline: c(0x283427),
        shadow: c(0x688f4e),
        coat: c(0xa7cb65),
        highlight: c(0xe1efaa),
        accent: c(0xf3a86b),
        eye: c(0x182017),
    },
    Palette {
        outline: c(0x3b2927),
        shadow: c(0x9c5d5a),
        coat: c(0xe8837b),
        highlight: c(0xffc1ae),
        accent: c(0x7ec8e3),
        eye: c(0x251817),
    },
    Palette {
        outline: c(0x233342),
        shadow: c(0x477fa8),
        coat: c(0x65b5df),
        highlight: c(0xbde8f6),
        accent: c(0xf5d76e),
        eye: c(0x14202a),
    },
    Palette {
        outline: c(0x35283d),
        shadow: c(0x87549d),
        coat: c(0xca78d1),
        highlight: c(0xf0b9ed),
        accent: c(0x84d6b4),
        eye: c(0x201726),
    },
    Palette {
        outline: c(0x3c3520),
        shadow: c(0xa38b42),
        coat: c(0xe0c65a),
        highlight: c(0xffeda0),
        accent: c(0xef7d77),
        eye: c(0x252014),
    },
    Palette {
        outline: c(0x2d3033),
        shadow: c(0x737b80),
        coat: c(0xaab3b7),
        highlight: c(0xe5ecec),
        accent: c(0xf49d6e),
        eye: c(0x1b1d1e),
    },
    Palette {
        outline: c(0x3b2a30),
        shadow: c(0x9c586b),
        coat: c(0xda7894),
        highlight: c(0xffb8c7),
        accent: c(0x94d3ac),
        eye: c(0x23181c),
    },
    Palette {
        outline: c(0x26352f),
        shadow: c(0x4f8b72),
        coat: c(0x70c49b),
        highlight: c(0xb9efd2),
        accent: c(0xbe87d9),
        eye: c(0x16221d),
    },
];
