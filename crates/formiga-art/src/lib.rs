mod bubble;
mod canvas;
mod card;
mod colony_card;
mod objects;
mod palette;
mod postcard;
mod renderer;
mod shelter;
mod sticker;
mod tree;
mod trinkets;
mod ui_atlas;

pub use bubble::MilestoneBubbleRenderer;
pub use canvas::{Canvas, Rgba};
pub use card::{CARD_HEIGHT, CARD_WIDTH, CreatureCardRenderer, abbreviated_seed_code};
pub use colony_card::{COLONY_CARD_HEIGHT, COLONY_CARD_WIDTH, ColonyCardRenderer};
pub type PixelCanvas = Canvas;
pub use objects::{
    COLONY_OBJECT_ATLAS_HEIGHT, COLONY_OBJECT_ATLAS_WIDTH, COLONY_OBJECT_CELLS, COLONY_OBJECT_SIZE,
    ColonyObjectRenderer,
};
pub use palette::{PALETTES, Palette, palette_for, prop_palette};
pub use postcard::{
    POSTCARD_CAPTION_LIMIT, POSTCARD_HEIGHT, POSTCARD_WIDTH, PostcardRenderer, PostcardScene,
    postcard_caption,
};
pub use renderer::{
    AlphaMask, AnimationAtlas, AnimationSpec, BodyClip, BodyPresentation, CreatureRenderer,
    DRINK_KINDS, ExpressionKind, EyelidPose, FACE_FRAME_SIZE, FRAME_SIZE, FaceRenderState,
    FramePlacement, GazeDirection, MotionSignature, PixelPoint, PlaybackMode, PropAnchor,
    RenderedBodyFrame, SNACK_KINDS, TOY_KINDS, prop_variants,
};
pub use shelter::{
    ResidentMark, SHELTER_SIZE, ShelterRenderer, VILLAGE_ATLAS_SIZE, VILLAGE_DAY_HEIGHT,
    VILLAGE_HOUSES, VillageCell,
};
pub use sticker::{
    DEFAULT_STICKER_SCALE, STICKER_SCALES, Sticker, StickerClip, StickerFrame, StickerRenderer,
};
pub use tree::{
    ANCHOR_CLEARANCE, KeepsakeTreeRenderer, TREE_CELL, TRINKET_ANCHORS, TrinketAnchor,
    trinket_place,
};
pub use trinkets::{
    TRINKET_ATLAS_BYTES, TRINKET_ATLAS_COLUMNS, TRINKET_ATLAS_HEIGHT, TRINKET_ATLAS_ROWS,
    TRINKET_ATLAS_WIDTH, TRINKET_CELL, TRINKET_FRAME_GLINT, TRINKET_FRAME_REST,
    TrinketAtlasRenderer, TrinketInk, draw_trinket,
};
pub use ui_atlas::{
    BUBBLE_ANCHOR, BUBBLE_CELL, LABEL_TAB_GAP, LABEL_TAB_HEIGHT, MENU_BODY_HEIGHT, MENU_CELL,
    MENU_ICON_BOX, MENU_MAX_ITEMS, MENU_MIN_ITEMS, MENU_NOTCH_HEIGHT, MENU_STRIP_HEIGHT, MenuIcon,
    MenuLayout, MenuRect, SpriteRect, UI_ATLAS_HEIGHT, UI_ATLAS_WIDTH, UiAtlasRenderer,
    menu_label_text,
};
