//! What a companion can bring back, and when.
//!
//! One table, read by the artwork, the scrapbook, the collection, and the simulation alike. The
//! variant number is the identifier a save keeps, so a variant always means what it meant when it
//! was first found: a colony that found a shell in an older version still has a shell here.
//! Variants 0..8 are the original everyday finds and 8..16 the first conditional ones; 16..160
//! were added in 0.60.0, grouped by the circumstance they turn up in.
//!
//! A `hint` is what an undiscovered slot says. It describes the trinket's own world — after dark,
//! high up, partway along a ride, beside a friend — and never asks the reader to go and do
//! anything. Nothing here is a task list.

use crate::TRINKET_VARIANTS;

/// The circumstance a trinket turns up in. `Anywhere` is the everyday finds, which have no
/// condition at all and can be found on any ordinary day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrinketCondition {
    Anywhere,
    /// Dark outside where the owner is: 20:00 until 06:00 local time.
    Night,
    /// On a window ledge with a long clear drop beneath it.
    HighTier,
    /// Riding a moving window, or just off one.
    MidRide,
    /// Standing beside a close friend.
    BesideCloseFriend,
    /// Found at the village while the houses are out.
    AtHome,
    /// Found while tending one of the village's gardens.
    InGarden,
    /// The early part of the day: 06:00 until 10:00 local time.
    Morning,
    /// Saturday or Sunday, local time.
    Weekend,
    /// Found on waking from a nap.
    AfterNap,
    /// Found sitting up on the roof of a house.
    OnRoof,
    /// Found at home while a guest is visiting.
    WithVisitor,
    /// After dark on a night the moon is full, or nearly.
    FullMoon,
    /// Within a few days of the colony's own birthday, once it has had one.
    ColonyBirthday,
    /// Anywhere, but only very rarely.
    Rare,
}

impl TrinketCondition {
    /// Every condition, in catalogue order.
    pub const ALL: [Self; 15] = [
        Self::Anywhere,
        Self::Night,
        Self::HighTier,
        Self::MidRide,
        Self::BesideCloseFriend,
        Self::AtHome,
        Self::InGarden,
        Self::Morning,
        Self::Weekend,
        Self::AfterNap,
        Self::OnRoof,
        Self::WithVisitor,
        Self::FullMoon,
        Self::ColonyBirthday,
        Self::Rare,
    ];

    /// How the collection groups what turns up in this circumstance.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Anywhere => "Everyday finds",
            Self::Night => "After dark",
            Self::HighTier => "High up",
            Self::MidRide => "Along a ride",
            Self::BesideCloseFriend => "Beside a friend",
            Self::AtHome => "Around the village",
            Self::InGarden => "In the garden",
            Self::Morning => "Early in the day",
            Self::Weekend => "At the weekend",
            Self::AfterNap => "After a nap",
            Self::OnRoof => "On a rooftop",
            Self::WithVisitor => "With a guest",
            Self::FullMoon => "Under a full moon",
            Self::ColonyBirthday => "Around the colony's birthday",
            Self::Rare => "Rare",
        }
    }
}

/// One trinket, as the scrapbook and the artwork both read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrinketInfo {
    /// The stable identifier a save keeps. Equal to this entry's index in the catalogue.
    pub variant: u8,
    pub name: &'static str,
    /// Shown once the trinket has been found.
    pub description: &'static str,
    /// Shown while the slot is still empty.
    pub hint: &'static str,
    pub condition: TrinketCondition,
}

/// Ordinary finds share one honest line: there is nothing to wait for.
const ANYWHERE_HINT: &str = "Could turn up on any ordinary day.";

const fn t(
    variant: u8,
    name: &'static str,
    description: &'static str,
    hint: &'static str,
    condition: TrinketCondition,
) -> TrinketInfo {
    TrinketInfo {
        variant,
        name,
        description,
        hint,
        condition,
    }
}

const fn everyday(variant: u8, name: &'static str, description: &'static str) -> TrinketInfo {
    t(
        variant,
        name,
        description,
        ANYWHERE_HINT,
        TrinketCondition::Anywhere,
    )
}

use TrinketCondition::*;

/// The catalogue. Variants 0..16 keep the names and descriptions the scrapbook has always shown.
static TRINKETS: [TrinketInfo; TRINKET_VARIANTS as usize] = [
    everyday(
        0,
        "Gem",
        "A cut stone that throws a little colour when the light moves.",
    ),
    everyday(
        1,
        "Key",
        "A small key. Nobody has found the lock it belongs to.",
    ),
    everyday(2, "Leaf", "A leaf pressed flat, kept for the shape of it."),
    everyday(
        3,
        "Shell",
        "A spiral shell, carried a long way from any sea.",
    ),
    everyday(
        4,
        "Ring charm",
        "A ring far too small for anyone here to wear.",
    ),
    everyday(
        5,
        "Tiny bottle",
        "A stoppered bottle with something cloudy inside.",
    ),
    everyday(
        6,
        "Star relic",
        "A little star of worn metal, its edges gone soft.",
    ),
    everyday(
        7,
        "Odd little tablet",
        "A flat tablet marked with lines nobody can read.",
    ),
    t(
        8,
        "Moon shard",
        "A sliver of pale stone that keeps a little light of its own.",
        "Only turns up after dark.",
        Night,
    ),
    t(
        9,
        "Firefly jar",
        "A small jar, still faintly warm, with nothing inside it now.",
        "Something from the late side of the evening.",
        Night,
    ),
    t(
        10,
        "Long feather",
        "A feather from something that passed by far overhead.",
        "Comes from somewhere high up.",
        HighTier,
    ),
    t(
        11,
        "Cloud puff",
        "A tuft of something soft that will not quite settle.",
        "Only found well above the ground.",
        HighTier,
    ),
    t(
        12,
        "Ticket stub",
        "Half a ticket, torn along the top, for a ride nobody remembers.",
        "Picked up partway along a ride.",
        MidRide,
    ),
    t(
        13,
        "Little pinwheel",
        "A paper wheel that spins whenever anything moves past it.",
        "Found mid-journey, never at either end.",
        MidRide,
    ),
    t(
        14,
        "Friendship knot",
        "A cord tied in a loop, with two ends that never come apart.",
        "Only found beside a close friend.",
        BesideCloseFriend,
    ),
    t(
        15,
        "Matching charms",
        "Two small charms cut from the same piece, kept together.",
        "Turns up when a dear friend is near.",
        BesideCloseFriend,
    ),
    // Everyday finds, added in 0.60.0.
    everyday(
        16,
        "Acorn",
        "An acorn with its cap still on, polished smooth by small paws.",
    ),
    everyday(
        17,
        "Button",
        "A four-holed button from a coat nobody here has ever worn.",
    ),
    everyday(
        18,
        "Marble",
        "A glass marble with a twist of colour caught in the middle.",
    ),
    everyday(
        19,
        "Thimble",
        "A thimble, just the right size for a hat, or a cup.",
    ),
    everyday(20, "Pinecone", "A pinecone, still shedding the odd scale."),
    everyday(
        21,
        "Bottle cap",
        "A crimped bottle cap, its paint worn to the metal at the edges.",
    ),
    everyday(
        22,
        "Paper boat",
        "A paper boat folded so neatly it has never been sailed.",
    ),
    everyday(
        23,
        "Ribbon",
        "A length of ribbon, curled from being wound round something.",
    ),
    everyday(
        24,
        "Scrap of wool",
        "A soft tangle of wool, the colour of a warm afternoon.",
    ),
    everyday(
        25,
        "Tiny bell",
        "A bell no bigger than a berry, with a bright little ring.",
    ),
    everyday(
        26,
        "Pressed daisy",
        "A daisy pressed flat between two pages and never taken out.",
    ),
    everyday(
        27,
        "Brass buckle",
        "A small brass buckle, the strap it held long gone.",
    ),
    everyday(
        28,
        "Patchwork square",
        "A square of patterned cloth with neat stitches round its edge.",
    ),
    everyday(
        29,
        "Comb",
        "A comb with three of its teeth missing, still very good.",
    ),
    everyday(
        30,
        "Clover",
        "A clover leaf, three-leafed and perfectly ordinary.",
    ),
    everyday(
        31,
        "Spool",
        "An empty wooden spool, good for rolling or standing on.",
    ),
    everyday(
        32,
        "Die",
        "A single die, worn round at the corners, that likes to land on six.",
    ),
    everyday(
        33,
        "Chalk stub",
        "A stub of chalk that leaves a pale line on anything.",
    ),
    everyday(
        34,
        "Pencil stub",
        "A pencil worn down to a stub, still sharp at one end.",
    ),
    everyday(
        35,
        "Holed coin",
        "A coin from nowhere anyone recognises, with a hole in the middle.",
    ),
    everyday(
        36,
        "Paperclip",
        "A paperclip bent into a shape that might be a heart.",
    ),
    everyday(
        37,
        "Safety pin",
        "A safety pin, closed tight around nothing at all.",
    ),
    everyday(
        38,
        "Sugar cube",
        "A sugar cube, too pretty to eat, which is saying something.",
    ),
    everyday(
        39,
        "Sweet wrapper",
        "A crinkly wrapper that still smells faintly of strawberries.",
    ),
    everyday(
        40,
        "Twig",
        "A forked twig with exactly the right bend in it.",
    ),
    everyday(
        41,
        "Shoelace",
        "A shoelace tied in a bow that nobody can quite undo.",
    ),
    everyday(
        42,
        "Holey pebble",
        "A pebble with a hole worn right through it.",
    ),
    everyday(43, "Walnut", "A walnut that rattles faintly when shaken."),
    everyday(
        44,
        "Toy block",
        "A wooden block with a letter painted on each side.",
    ),
    everyday(45, "Domino", "A single domino with two dots and a scratch."),
    everyday(
        46,
        "Chess pawn",
        "A small carved pawn, a long way from its board.",
    ),
    everyday(
        47,
        "Crayon",
        "A crayon in a colour somewhere between two other colours.",
    ),
    everyday(
        48,
        "Eraser",
        "A soft pink eraser that has rubbed out a great many mistakes.",
    ),
    everyday(
        49,
        "Postage stamp",
        "A stamp from a letter that went somewhere far away.",
    ),
    everyday(
        50,
        "Rubber band",
        "A rubber band, stretched just a little out of shape.",
    ),
    everyday(
        51,
        "Cork",
        "A bottle cork with a little star pressed into its top.",
    ),
    everyday(
        52,
        "Glass bead",
        "A glass bead, one of a necklace that came undone.",
    ),
    everyday(
        53,
        "Tin whistle",
        "A tin whistle that plays only one note, and plays it well.",
    ),
    everyday(
        54,
        "Tiny mitten",
        "A single tiny mitten. The other is still out there somewhere.",
    ),
    everyday(
        55,
        "Teaspoon",
        "A teaspoon with a flower stamped on its handle.",
    ),
    // After dark.
    t(
        56,
        "Candle stub",
        "The end of a candle, with a wick that remembers being lit.",
        "Found once the lamps come on.",
        Night,
    ),
    t(
        57,
        "Owl feather",
        "A soft, silent feather, barred in grey.",
        "Dropped by something that flies at night.",
        Night,
    ),
    t(
        58,
        "Fallen star",
        "A crumb of something bright that fell a very long way.",
        "Comes down on dark evenings.",
        Night,
    ),
    t(
        59,
        "Moth wing",
        "A dusty moth wing, soft as a whisper.",
        "Something from after dark.",
        Night,
    ),
    t(
        60,
        "Amber pane",
        "A little pane of amber glass that glows when held up.",
        "Turns up in the evening light.",
        Night,
    ),
    t(
        61,
        "Dream catcher",
        "A hoop of thread woven like a web, with a bead caught in it.",
        "Belongs to the hours of sleep.",
        Night,
    ),
    t(
        62,
        "Glow mushroom",
        "A small mushroom that glows a faint green in the dark.",
        "Only shows itself after nightfall.",
        Night,
    ),
    t(
        63,
        "Nightcap tassel",
        "The tassel off a nightcap, a little squashed from sleeping on.",
        "Something from the far side of bedtime.",
        Night,
    ),
    t(
        64,
        "Meteorite",
        "A dark, heavy pebble, pitted as though it was once very hot.",
        "Falls out of a night sky.",
        Night,
    ),
    t(
        65,
        "Silver thread",
        "A silver thread that catches moonlight better than anything.",
        "Glints only in the dark.",
        Night,
    ),
    // High up.
    t(
        66,
        "Kite tail",
        "A ribbon from a kite's tail, still fluttering.",
        "Caught somewhere high.",
        HighTier,
    ),
    t(
        67,
        "Eggshell",
        "Half a speckled eggshell, empty and light as air.",
        "From a nest far above the ground.",
        HighTier,
    ),
    t(
        68,
        "Seed wing",
        "A sycamore seed that spins down like a little helicopter.",
        "Drifts down from somewhere lofty.",
        HighTier,
    ),
    t(
        69,
        "Balloon string",
        "A string that once held a balloon, and still tugs upward.",
        "Snagged well above the floor.",
        HighTier,
    ),
    t(
        70,
        "Paper snowflake",
        "A paper snowflake that never melts.",
        "Found up where the air is thin.",
        HighTier,
    ),
    t(
        71,
        "Dandelion clock",
        "A dandelion clock with every seed still on it.",
        "Floats about well above the ground.",
        HighTier,
    ),
    t(
        72,
        "Paper plane",
        "A paper plane with a very long flight behind it.",
        "Lands only in high places.",
        HighTier,
    ),
    t(
        73,
        "Soap bubble",
        "A soap bubble that has somehow not popped yet.",
        "Hangs about at a great height.",
        HighTier,
    ),
    // Along a ride.
    t(
        74,
        "Map scrap",
        "A torn corner of a map, with half a road on it.",
        "Picked up along the way.",
        MidRide,
    ),
    t(
        75,
        "Compass",
        "A tiny compass whose needle wobbles before it settles.",
        "Turns up while going somewhere.",
        MidRide,
    ),
    t(
        76,
        "Postcard",
        "A postcard with a view of somewhere, and no message.",
        "Something from a journey in motion.",
        MidRide,
    ),
    t(
        77,
        "Luggage tag",
        "A luggage tag with a name smudged too much to read.",
        "Found in the middle of a ride.",
        MidRide,
    ),
    t(
        78,
        "Toy wheel",
        "A little wheel off a toy that got somewhere in a hurry.",
        "Rolls up partway along a ride.",
        MidRide,
    ),
    t(
        79,
        "Sweet tin",
        "A small round tin with one boiled sweet rattling inside.",
        "Something for the journey.",
        MidRide,
    ),
    // Beside a close friend.
    t(
        80,
        "Heart pebble",
        "A pebble worn into a heart, by chance or by patience.",
        "Found in good company.",
        BesideCloseFriend,
    ),
    t(
        81,
        "Half a biscuit",
        "Half a biscuit, carefully saved for someone.",
        "Turns up between close friends.",
        BesideCloseFriend,
    ),
    t(
        82,
        "Twin cherries",
        "Two cherries joined at the stalk, too sweet to split.",
        "Only found with a dear friend near.",
        BesideCloseFriend,
    ),
    t(
        83,
        "Locket",
        "A locket that opens onto two tiny portraits of nobody in particular.",
        "Belongs to a close friendship.",
        BesideCloseFriend,
    ),
    t(
        84,
        "Paper chain",
        "A paper chain of little figures holding hands.",
        "Found side by side with a friend.",
        BesideCloseFriend,
    ),
    t(
        85,
        "Tin telephone",
        "Two tins and a string, for talking at a distance.",
        "Turns up where two friends stand together.",
        BesideCloseFriend,
    ),
    t(
        86,
        "Wishbone",
        "A wishbone that nobody could bring themselves to pull.",
        "Found in the company of a close friend.",
        BesideCloseFriend,
    ),
    t(
        87,
        "Woven bracelet",
        "A bracelet woven from two colours, twisted together.",
        "Turns up when friends are close.",
        BesideCloseFriend,
    ),
    // Around the village.
    t(
        88,
        "Doorknob",
        "A round brass doorknob that opens nothing now.",
        "Turns up around the village.",
        AtHome,
    ),
    t(
        89,
        "Teacup",
        "A tiny teacup without a saucer, chipped on the rim.",
        "Found near the houses.",
        AtHome,
    ),
    t(
        90,
        "Clothes peg",
        "A wooden clothes peg with a spring that still snaps.",
        "Something from home.",
        AtHome,
    ),
    t(
        91,
        "Little broom",
        "A little broom, worn at the bristles from sweeping the step.",
        "Turns up by a doorstep.",
        AtHome,
    ),
    t(
        92,
        "Leaf candle holder",
        "A candle holder shaped like a leaf, with a little finger loop.",
        "Found around the houses.",
        AtHome,
    ),
    t(
        93,
        "Jam jar lid",
        "A jam jar lid with a gingham pattern round its edge.",
        "Turns up at home.",
        AtHome,
    ),
    t(
        94,
        "Knitting needle",
        "One knitting needle, with a row of stitches still on it.",
        "Something from indoors.",
        AtHome,
    ),
    t(
        95,
        "Cake crumb",
        "A crumb of cake, saved for later and then forgotten.",
        "Found about the village.",
        AtHome,
    ),
    t(
        96,
        "Little brick",
        "A little brick, one corner rounded from being carried.",
        "Turns up among the houses.",
        AtHome,
    ),
    t(
        97,
        "Ember stone",
        "A smooth stone that stays warm long after the sun goes.",
        "Found close to home.",
        AtHome,
    ),
    t(
        98,
        "Recipe card",
        "A recipe card for soup, with one step smudged by a thumb.",
        "Something from a kitchen.",
        AtHome,
    ),
    t(
        99,
        "Door number",
        "A brass number seven, fallen off a door somewhere.",
        "Turns up near a front door.",
        AtHome,
    ),
    t(
        100,
        "Chime bar",
        "One bar of a wind chime, which still rings on its own.",
        "Found hanging about the village.",
        AtHome,
    ),
    t(
        101,
        "Tea tag",
        "The paper tag off a tea bag, with a kind word printed on it.",
        "Turns up at teatime, at home.",
        AtHome,
    ),
    t(
        102,
        "Pincushion",
        "A pincushion like a tomato, with two pins still in it.",
        "Something from a sewing basket.",
        AtHome,
    ),
    t(
        103,
        "Slipper",
        "A single slipper, soft and very small.",
        "Found just inside a door.",
        AtHome,
    ),
    // In the garden.
    t(
        104,
        "Seed packet",
        "A paper packet of seeds that rattle like a promise.",
        "Turns up in the garden.",
        InGarden,
    ),
    t(
        105,
        "Snail shell",
        "An empty snail shell, spiralled the other way from most.",
        "Found among the plants.",
        InGarden,
    ),
    t(
        106,
        "Ladybird button",
        "A red button with black spots that looks very like a ladybird.",
        "Something from between the leaves.",
        InGarden,
    ),
    t(
        107,
        "Rose petal",
        "A rose petal, velvety and a little curled.",
        "Falls in the flower beds.",
        InGarden,
    ),
    t(
        108,
        "Tiny toadstool",
        "A little red toadstool, spotted white.",
        "Comes up in the garden.",
        InGarden,
    ),
    t(
        109,
        "Carrot top",
        "The feathery top of a carrot, kept for its green.",
        "Found in the vegetable rows.",
        InGarden,
    ),
    t(
        110,
        "Pea pod",
        "A pea pod with five peas lined up inside.",
        "Something from the garden.",
        InGarden,
    ),
    t(
        111,
        "Garden twine",
        "A loop of garden twine, for tying up whatever needs it.",
        "Turns up by the garden beds.",
        InGarden,
    ),
    t(
        112,
        "Radish",
        "A radish no bigger than a bead, bright red and peppery.",
        "Pulled from a garden patch.",
        InGarden,
    ),
    t(
        113,
        "Sprinkler rose",
        "The sprinkler head off a watering can.",
        "Found wherever the garden gets watered.",
        InGarden,
    ),
    t(
        114,
        "Honeycomb",
        "A little piece of honeycomb, still sticky.",
        "Turns up where the bees go.",
        InGarden,
    ),
    t(
        115,
        "Plant label",
        "A wooden plant label with a name on it that nobody can read.",
        "Stuck in the soil somewhere.",
        InGarden,
    ),
    // Early in the day.
    t(
        116,
        "Dewdrop",
        "A dewdrop that has somehow kept its shape.",
        "Found in the early morning.",
        Morning,
    ),
    t(
        117,
        "Buttercup",
        "A buttercup that throws yellow light under a chin.",
        "Turns up while the day is new.",
        Morning,
    ),
    t(
        118,
        "Egg cup",
        "An egg cup painted with a small sun.",
        "Something from breakfast time.",
        Morning,
    ),
    t(
        119,
        "Toast crust",
        "A toast crust, golden and crunchy.",
        "Found in the first hours of the day.",
        Morning,
    ),
    t(
        120,
        "Morning glory",
        "A morning glory flower, open wide.",
        "Opens only in the morning.",
        Morning,
    ),
    t(
        121,
        "Clock key",
        "The winding key from a clock that rings too early.",
        "Turns up at the start of the day.",
        Morning,
    ),
    t(
        122,
        "Foil bottle top",
        "A foil top from a milk bottle, pressed smooth.",
        "Arrives with the early deliveries.",
        Morning,
    ),
    t(
        123,
        "Sunbeam glass",
        "A chip of glass that throws a small sunbeam wherever it points.",
        "Catches the morning sun.",
        Morning,
    ),
    // At the weekend.
    t(
        124,
        "Diamond kite",
        "A little diamond kite with a bow-tied tail.",
        "Only turns up at the weekend.",
        Weekend,
    ),
    t(
        125,
        "Checked napkin",
        "A folded napkin in red and white checks.",
        "Found on a slow Saturday or Sunday.",
        Weekend,
    ),
    t(
        126,
        "Yo-yo",
        "A wooden yo-yo that comes back up every time.",
        "Something for a day off.",
        Weekend,
    ),
    t(
        127,
        "Comic strip",
        "A folded page of a comic, the funny bit worn soft.",
        "Turns up at the weekend.",
        Weekend,
    ),
    t(
        128,
        "Jam tart",
        "A jam tart with a star cut in its lid.",
        "Found on a lazy weekend.",
        Weekend,
    ),
    t(
        129,
        "Bubble wand",
        "A bubble wand, still a little soapy.",
        "Belongs to Saturdays and Sundays.",
        Weekend,
    ),
    // After a nap.
    t(
        130,
        "Pyjama button",
        "A small button shaped like a moon, off somebody's pyjamas.",
        "Found on waking from a nap.",
        AfterNap,
    ),
    t(
        131,
        "Blanket fluff",
        "A ball of blanket fluff, gathered over many naps.",
        "Turns up after a good sleep.",
        AfterNap,
    ),
    t(
        132,
        "Toy sheep",
        "A little wooden sheep, the one that gets counted last.",
        "Something left over from a dream.",
        AfterNap,
    ),
    t(
        133,
        "Striped sock",
        "One small striped sock. It was under the pillow.",
        "Found just after waking.",
        AfterNap,
    ),
    t(
        134,
        "Dream jar",
        "A jar that holds a dream, for anyone who believes it does.",
        "Turns up at the end of a nap.",
        AfterNap,
    ),
    t(
        135,
        "Pocket watch",
        "A pocket watch that is always five minutes slow.",
        "Found on the way out of a snooze.",
        AfterNap,
    ),
    // On a rooftop.
    t(
        136,
        "Weathervane arrow",
        "A little tin arrow that always points into the wind.",
        "Found up on a rooftop.",
        OnRoof,
    ),
    t(
        137,
        "Tiny flag",
        "A tiny flag on a pin, for marking the top of something.",
        "Turns up at the top of a house.",
        OnRoof,
    ),
    t(
        138,
        "Roof moss",
        "A tuft of soft green moss from between the roof tiles.",
        "Grows on the roofs.",
        OnRoof,
    ),
    t(
        139,
        "Shingle",
        "A wooden shingle, curled at the corner by the sun.",
        "Found on a roof.",
        OnRoof,
    ),
    t(
        140,
        "Telescope lens",
        "A round lens, for seeing things that are far off.",
        "Something with a view.",
        OnRoof,
    ),
    t(
        141,
        "Nest twig",
        "A twig from an old nest, bent into a curl.",
        "Turns up on a rooftop.",
        OnRoof,
    ),
    // With a guest.
    t(
        142,
        "Guest ribbon",
        "A ribbon a visitor wore on their way here.",
        "Turns up while a guest is visiting.",
        WithVisitor,
    ),
    t(
        143,
        "Foreign coin",
        "A coin from wherever the last visitor came from.",
        "Arrives with company.",
        WithVisitor,
    ),
    t(
        144,
        "Gift bow",
        "A bow off a present that somebody brought.",
        "Found when someone comes to call.",
        WithVisitor,
    ),
    t(
        145,
        "Thank-you note",
        "A folded note that just says 'thank you'.",
        "Left behind by a guest.",
        WithVisitor,
    ),
    t(
        146,
        "Walking stick",
        "A walking stick, just the right height for someone small.",
        "Turns up with a visitor.",
        WithVisitor,
    ),
    t(
        147,
        "Pearl hat pin",
        "A long pin with a pearl on the end, from a visitor's hat.",
        "Found while the village has company.",
        WithVisitor,
    ),
    // Under a full moon.
    t(
        148,
        "Moonstone",
        "A milky stone with a blue gleam, like the moon on water.",
        "Only under a full moon.",
        FullMoon,
    ),
    t(
        149,
        "Silver bell",
        "A silver bell that rings very softly by itself.",
        "Turns up on the brightest nights.",
        FullMoon,
    ),
    t(
        150,
        "Moonflower",
        "A moonflower, which only opens when the moon is full.",
        "Opens under a round moon.",
        FullMoon,
    ),
    t(
        151,
        "Hare charm",
        "A little silver hare, the kind that lives in the moon.",
        "Found when the moon is full.",
        FullMoon,
    ),
    // Around the colony's birthday.
    t(
        152,
        "Party streamer",
        "A curl of paper streamer from a party.",
        "Turns up around the colony's birthday.",
        ColonyBirthday,
    ),
    t(
        153,
        "Birthday candle",
        "A striped birthday candle with a burnt wick.",
        "Found once a year, near the colony's birthday.",
        ColonyBirthday,
    ),
    t(
        154,
        "Slice of cake",
        "A slice of cake with a candle-hole in it.",
        "Something from a birthday party.",
        ColonyBirthday,
    ),
    t(
        155,
        "Confetti",
        "A pinch of confetti in every colour.",
        "Falls on the colony's birthday.",
        ColonyBirthday,
    ),
    // Rare.
    t(
        156,
        "Golden acorn",
        "An acorn made of gold, or painted so well it hardly matters.",
        "Very rarely turns up, anywhere at all.",
        Rare,
    ),
    t(
        157,
        "Four-leaf clover",
        "A clover with four leaves. This one has been checked twice.",
        "Once in a long while, anywhere.",
        Rare,
    ),
    t(
        158,
        "Crystal feather",
        "A feather made of glass, cool and faintly ringing.",
        "Hardly ever found.",
        Rare,
    ),
    t(
        159,
        "Rainbow marble",
        "A marble with a whole rainbow folded up inside it.",
        "The rarest kind of find.",
        Rare,
    ),
];

/// The catalogue entry for one variant, or `None` for a number no artwork exists for.
pub fn trinket_info(variant: u8) -> Option<&'static TrinketInfo> {
    TRINKETS.get(usize::from(variant))
}

/// Every trinket found in one circumstance, in catalogue order.
pub fn trinkets_for(condition: TrinketCondition) -> impl Iterator<Item = &'static TrinketInfo> {
    TRINKETS.iter().filter(move |t| t.condition == condition)
}

/// Every trinket, in catalogue order.
pub fn all_trinkets() -> impl Iterator<Item = &'static TrinketInfo> {
    TRINKETS.iter()
}

/// How many trinkets turn up in one circumstance.
pub fn trinket_count(condition: TrinketCondition) -> usize {
    trinkets_for(condition).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_catalogue_covers_every_variant_exactly_once() {
        assert_eq!(TRINKETS.len(), 160);
        assert_eq!(TRINKETS.len(), usize::from(TRINKET_VARIANTS));
        for (index, info) in TRINKETS.iter().enumerate() {
            assert_eq!(
                usize::from(info.variant),
                index,
                "variants are contiguous and equal to their own index"
            );
            assert_eq!(trinket_info(info.variant), Some(info));
        }
        assert_eq!(trinket_info(TRINKET_VARIANTS), None);
        assert_eq!(trinket_info(u8::MAX), None);
    }

    #[test]
    fn the_original_sixteen_keep_their_numbers_names_and_circumstances() {
        // A save from before this table still means what it meant.
        for variant in 0..8 {
            let info = trinket_info(variant).expect("the original eight are still here");
            assert_eq!(info.condition, TrinketCondition::Anywhere, "{variant}");
        }
        assert_eq!(trinket_info(0).unwrap().name, "Gem");
        assert_eq!(trinket_info(7).unwrap().name, "Odd little tablet");
        assert_eq!(trinket_info(8).unwrap().name, "Moon shard");
        assert_eq!(trinket_info(15).unwrap().name, "Matching charms");
        for (variant, condition) in [
            (8, TrinketCondition::Night),
            (9, TrinketCondition::Night),
            (10, TrinketCondition::HighTier),
            (11, TrinketCondition::HighTier),
            (12, TrinketCondition::MidRide),
            (13, TrinketCondition::MidRide),
            (14, TrinketCondition::BesideCloseFriend),
            (15, TrinketCondition::BesideCloseFriend),
        ] {
            assert_eq!(trinket_info(variant).unwrap().condition, condition);
        }
        assert_eq!(
            TrinketCondition::ALL
                .into_iter()
                .map(trinket_count)
                .sum::<usize>(),
            TRINKETS.len(),
            "every trinket belongs to exactly one condition"
        );
        // Every circumstance has something to find, and the everyday finds are the biggest group.
        for condition in TrinketCondition::ALL {
            assert!(trinket_count(condition) >= 4, "{condition:?}");
        }
        assert_eq!(trinket_count(TrinketCondition::Anywhere), 48);
    }

    #[test]
    fn every_trinket_reads_as_its_own_thing() {
        let names: BTreeSet<&str> = TRINKETS.iter().map(|t| t.name).collect();
        assert_eq!(names.len(), TRINKETS.len(), "names are unique");
        let descriptions: BTreeSet<&str> = TRINKETS.iter().map(|t| t.description).collect();
        assert_eq!(
            descriptions.len(),
            TRINKETS.len(),
            "descriptions are unique"
        );
        for info in &TRINKETS {
            for text in [info.name, info.description, info.hint] {
                assert!(!text.trim().is_empty(), "{info:?} has an empty line");
                assert!(text.len() <= 80, "{text:?} is too long for a card");
            }
            assert!(
                info.name.len() <= 20,
                "{:?} is too long for a label",
                info.name
            );
            // A hint describes where a thing lives. It never tells the reader to change what they
            // are doing with their own computer.
            let hint = info.hint.to_ascii_lowercase();
            for instruction in [
                "you ",
                "your ",
                "try ",
                "keep ",
                "leave ",
                "wait ",
                "make sure",
            ] {
                assert!(
                    !hint.contains(instruction),
                    "{:?} instructs the reader: {:?}",
                    info.name,
                    info.hint
                );
            }
        }
        // Conditional slots say something about their own circumstance rather than nothing.
        for info in TRINKETS
            .iter()
            .filter(|t| t.condition != TrinketCondition::Anywhere)
        {
            assert_ne!(
                info.hint, ANYWHERE_HINT,
                "{:?} needs its own hint",
                info.name
            );
        }
    }
}
