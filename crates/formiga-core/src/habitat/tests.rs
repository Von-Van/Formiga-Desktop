use super::*;
use crate::{ColonyHome, DisplayKey, HabitatZone, MonitorInfo};

/// Room enough for the longest strip a colony can grow at any supported scale, so a test
/// about where things sit is never really a test about what did not fit.
fn wide_monitor(scale_factor: f32) -> MonitorInfo {
    MonitorInfo {
        id: 7,
        display_key: DisplayKey([9; 16]),
        bounds: DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 3840.0,
            height: 1200.0,
        },
        usable_bounds: DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 3840.0,
            height: 1200.0,
        },
        scale_factor,
        primary: true,
    }
}

fn village_home(monitor: &MonitorInfo, corner: HomeCorner) -> ColonyHome {
    let mut home = ColonyHome::from_seed([13; 32], Some(monitor.display_key), None, None);
    home.corner = corner;
    home
}

/// Houses drawn at their present size must not cost anybody their home. On the displays people
/// actually have, at every creature size, a full six-companion colony shows at least as many
/// houses as it did with houses a fifth smaller — the numbers below are from then — laying the
/// village out snug where the comfortable layout would leave a house off the end. A snug
/// village gives up a keepsake tree before a house, never the other way round.
#[test]
fn a_full_colony_shows_every_house_it_did_before_the_houses_grew() {
    let cottages = widest_colony();
    // Width, usable height, scale factor, and the houses shown at 2x, 3x and 4x when they were
    // a fifth smaller.
    let displays: [(f32, f32, f32, [usize; 3]); 9] = [
        (1280.0, 760.0, 1.0, [6, 6, 4]),
        (1366.0, 728.0, 1.0, [6, 6, 4]),
        (1440.0, 860.0, 1.0, [6, 6, 5]),
        (1536.0, 824.0, 1.0, [6, 6, 5]),
        (1920.0, 1040.0, 1.0, [6, 6, 6]),
        (1280.0, 775.0, 2.0, [6, 6, 6]),
        (1440.0, 875.0, 2.0, [6, 6, 6]),
        (1512.0, 950.0, 2.0, [6, 6, 6]),
        (1280.0, 680.0, 1.5, [6, 6, 6]),
    ];
    for (width, height, scale_factor, before) in displays {
        let monitor = MonitorInfo {
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            ..wide_monitor(scale_factor)
        };
        let monitors = std::slice::from_ref(&monitor);
        let policy = HabitatPolicy::default();
        for (display_scale, shown_before) in (2_u8..=4).zip(before) {
            for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                let home = village_home(&monitor, corner);
                let shown: Vec<usize> = (0..=cottages.len())
                    .filter(|slot| {
                        home_dwelling_position(
                            &home,
                            *slot,
                            &cottages,
                            monitors,
                            &policy,
                            display_scale,
                        )
                        .is_some()
                    })
                    .collect();
                assert!(
                    shown.len() >= shown_before,
                    "{width}x{height}@{scale_factor} at {display_scale}x, {corner:?}: \
                     {} houses where 0.64.0 showed {shown_before}",
                    shown.len()
                );
                // The houses shown are always the first ones along, never a gap in the row.
                assert_eq!(shown, (0..shown.len()).collect::<Vec<_>>());
                // And every lot that is shown is inside the display.
                let ground =
                    VillageGround::resolve(&home, &cottages, monitors, &policy, display_scale)
                        .unwrap();
                let unit = ground.scale;
                for slot in &shown {
                    let (_, point) = home_dwelling_position(
                        &home,
                        *slot,
                        &cottages,
                        monitors,
                        &policy,
                        display_scale,
                    )
                    .unwrap();
                    let half =
                        ground.lot_width(VillageLot::Dwelling(*slot), &cottages) / 2.0 * unit;
                    assert!(point.x - half >= -0.01 && point.x + half <= width + 0.01);
                }
            }
        }
    }
}

/// The shortcut that decides whether every house fits gives the same answer as laying the
/// walk out and placing each house, on every display, size, colony and corner tried.
#[test]
fn the_quick_fit_check_agrees_with_placing_every_house() {
    let policy = HabitatPolicy::default();
    for width in [
        700.0_f32, 900.0, 1100.0, 1280.0, 1366.0, 1440.0, 1920.0, 3840.0,
    ] {
        for scale_factor in [1.0_f32, 1.5, 2.0] {
            let monitor = MonitorInfo {
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height: 800.0,
                },
                usable_bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height: 800.0,
                },
                ..wide_monitor(scale_factor)
            };
            for houses in 0..crate::MAX_COLONY_CREATURES {
                let cottages = vec![DwellingKind::Cottage; houses];
                for display_scale in 1..=4 {
                    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
                        let home = village_home(&monitor, corner);
                        for fit in [VillageFit::Comfortable, VillageFit::Snug] {
                            let Some(ground) = VillageGround::resolve_fit(
                                &home,
                                std::slice::from_ref(&monitor),
                                &policy,
                                display_scale,
                                fit,
                            ) else {
                                continue;
                            };
                            assert_eq!(
                                ground.shows_every_house(&cottages),
                                ground.houses_shown(&cottages) == houses + 1,
                                "{width}@{scale_factor} {houses} cottages at {display_scale}x \
                                 {corner:?} {fit:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Laid out snug, the houses stand shoulder to shoulder and never on one another, and the
/// village is only snug where the comfortable layout would have left a house off.
#[test]
fn a_snug_village_keeps_its_houses_apart_and_is_only_snug_where_it_must_be() {
    let cottages = widest_colony();
    let narrow = MonitorInfo {
        bounds: DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 760.0,
        },
        usable_bounds: DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 760.0,
        },
        ..wide_monitor(1.0)
    };
    let policy = HabitatPolicy::default();
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        let home = village_home(&narrow, corner);
        let ground =
            VillageGround::resolve(&home, &cottages, std::slice::from_ref(&narrow), &policy, 3)
                .unwrap();
        assert_eq!(ground.fit, VillageFit::Snug, "{corner:?}");
        let lots = placed_lots(&home, &cottages, &narrow, 3);
        let houses: Vec<_> = lots
            .iter()
            .filter(|(lot, ..)| matches!(lot, VillageLot::Dwelling(_)))
            .collect();
        assert_eq!(houses.len(), cottages.len() + 1);
        for pair in houses.windows(2) {
            let ((_, a, a_half), (_, b, b_half)) = (pair[0], pair[1]);
            assert!(
                a + a_half <= b - b_half + 0.01,
                "{corner:?}: houses overlap"
            );
        }
        // The outward tree gave up its ground for the houses.
        assert!(
            home_tree_position(
                &home,
                TreeEnd::Outward,
                &cottages,
                std::slice::from_ref(&narrow),
                &policy,
                3
            )
            .is_none()
        );
        // A smaller colony on the same display has room enough to be comfortable.
        let ground = VillageGround::resolve(
            &home,
            &small_colony(),
            std::slice::from_ref(&narrow),
            &policy,
            3,
        )
        .unwrap();
        assert_eq!(ground.fit, VillageFit::Comfortable, "{corner:?}");
    }
    // A wide display never lays a village out snug.
    let wide = wide_monitor(1.0);
    for scale in 1..=4 {
        let ground = VillageGround::resolve(
            &village_home(&wide, HomeCorner::BottomLeft),
            &cottages,
            std::slice::from_ref(&wide),
            &policy,
            scale,
        )
        .unwrap();
        assert_eq!(ground.fit, VillageFit::Comfortable);
    }
}

/// A band along the bottom of the display — the Bottom edge habitat, above a Dock or a taskbar
/// — that held a full village while the houses were a fifth smaller still holds every house and
/// both trees, at the sizes it did: Medium on a 1080p display, Small on a 768-line laptop, and
/// Large on a Retina laptop. The houses are fitted by how tall they are drawn, not by the empty
/// top of the cell they are drawn from.
#[test]
fn a_bottom_band_that_held_the_village_before_the_houses_grew_still_holds_it() {
    for (width, height, scale_factor, display_scale) in [
        (1920.0, 1040.0, 1.0, 3_u8),
        (1366.0, 728.0, 1.0, 2),
        (1440.0, 875.0, 2.0, 4),
    ] {
        let monitor = MonitorInfo {
            bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            usable_bounds: DesktopRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            ..wide_monitor(scale_factor)
        };
        let monitors = std::slice::from_ref(&monitor);
        let policy = HabitatPolicy {
            preset: HabitatPreset::BottomEdge,
            zones: Vec::new(),
        };
        let cottages = widest_colony();
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            for slot in 0..=cottages.len() {
                assert!(
                    home_dwelling_position(
                        &home,
                        slot,
                        &cottages,
                        monitors,
                        &policy,
                        display_scale
                    )
                    .is_some(),
                    "{width}x{height} at {display_scale}x, {corner:?}: house {slot} is hidden"
                );
            }
            for end in TreeEnd::BOTH {
                assert!(
                    home_tree_position(&home, end, &cottages, monitors, &policy, display_scale)
                        .is_some(),
                    "{width}x{height} at {display_scale}x, {corner:?}: the {end:?} tree is hidden"
                );
            }
        }
    }
}

/// The widest village there is: four adults, every house at its full footprint. Every rule
/// about ground and clearance is tightest here, so this is the colony the layout is checked
/// against rather than every shape it can take.
/// The village at its widest: a house for every one of six full-size companions.
fn widest_colony() -> Vec<DwellingKind> {
    vec![DwellingKind::Cottage; crate::MAX_COLONY_CREATURES - 1]
}

/// A colony part-way there, which should take correspondingly less ground.
fn small_colony() -> Vec<DwellingKind> {
    vec![DwellingKind::Cottage; 2]
}

/// The tightest drawing the overlay does: the largest scale on the display that magnifies it
/// least, so one shelter pixel is four desktop points and every clearance is at its coarsest.
const TIGHTEST_SCALE: u8 = 4;

/// Every lot the village lays out, as `(lot, centre, half width)` in desktop points measured
/// outward from the colony house, so both corners can be compared as one set of numbers.
fn placed_lots(
    home: &ColonyHome,
    cottages: &[DwellingKind],
    monitor: &MonitorInfo,
    display_scale: u8,
) -> Vec<(VillageLot, f32, f32)> {
    let policy = HabitatPolicy::default();
    let monitors = std::slice::from_ref(monitor);
    let anchor = resolved_home_anchor(home, cottages, monitor, display_scale, &policy).unwrap();
    let ground = VillageGround::resolve(home, cottages, monitors, &policy, display_scale).unwrap();
    let unit = f32::from(display_scale) / monitor.scale_factor.max(1.0);
    let direction = if home.corner == HomeCorner::BottomLeft {
        1.0
    } else {
        -1.0
    };
    ground
        .walk(cottages)
        .iter()
        .filter_map(|(lot, _)| {
            let point = match lot {
                VillageLot::Tree(end) => {
                    home_tree_position(home, end, cottages, monitors, &policy, display_scale)
                }
                VillageLot::Dwelling(slot) => {
                    home_dwelling_position(home, slot, cottages, monitors, &policy, display_scale)
                }
            }?
            .1;
            assert!(
                (point.y - anchor.y).abs() <= unit + 0.01,
                "{lot:?} left the ground line"
            );
            Some((
                lot,
                (point.x - anchor.x) * direction,
                ground.lot_width(lot, cottages) / 2.0 * unit,
            ))
        })
        .collect()
}

/// The houses and both trees come out of one walk, so no two houses may ever claim the same
/// ground and a tree reaches in over its end house by `TREE_OVERLAP` and no further — and the
/// two corners have to be the same village, measured from the house outward.
#[test]
fn the_houses_and_the_trees_share_one_walk_and_mirror_in_both_corners() {
    let monitor = wide_monitor(2.0);
    let cottages = widest_colony();
    let mut mirrored: Option<Vec<(VillageLot, f32, f32)>> = None;
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        let home = village_home(&monitor, corner);
        let lots = placed_lots(&home, &cottages, &monitor, TIGHTEST_SCALE);
        assert_eq!(
            lots.len(),
            3 + cottages.len(),
            "two trees and a house each should all fit a display this size"
        );
        // The bookends: one past the colony house away from the cottages, one past the last
        // cottage, with every house between them.
        let tree_of = |end| {
            lots.iter()
                .find(|(lot, _, _)| *lot == VillageLot::Tree(end))
                .map(|(_, centre, _)| *centre)
                .expect("both trees fit a display this size")
        };
        let (outward, inward) = (tree_of(TreeEnd::Outward), tree_of(TreeEnd::Inward));
        assert!(
            outward < -DwellingKind::Main.width() / 2.0,
            "the outward tree stands past the colony house, away from the cottages"
        );
        for (lot, centre, _) in &lots {
            if matches!(lot, VillageLot::Tree(_)) {
                continue;
            }
            assert!(
                (outward..=inward).contains(centre),
                "{lot:?} stands outside the two trees"
            );
        }
        let unit = f32::from(TIGHTEST_SCALE) / monitor.scale_factor.max(1.0);
        for (index, (lot, centre, half)) in lots.iter().enumerate() {
            for (other, other_centre, other_half) in &lots[index + 1..] {
                let clearance = (centre - other_centre).abs();
                let tree_over_house = matches!(
                    (lot, other),
                    (VillageLot::Tree(_), VillageLot::Dwelling(_))
                        | (VillageLot::Dwelling(_), VillageLot::Tree(_))
                );
                let overlap = if tree_over_house {
                    TREE_OVERLAP * unit
                } else {
                    0.0
                };
                assert!(
                    clearance >= half + other_half - overlap - 0.01,
                    "{corner:?}: {lot:?} and {other:?} share ground ({clearance} apart, \
                     {half} + {other_half} wide)"
                );
            }
        }
        match &mirrored {
            None => mirrored = Some(lots),
            Some(first) => {
                assert_eq!(first.len(), lots.len());
                for ((lot, a, _), (other, b, _)) in first.iter().zip(&lots) {
                    assert_eq!(lot, other);
                    assert!(
                        (a - b).abs() < 0.01,
                        "{lot:?} sits {a} out in one corner and {b} in the other"
                    );
                }
            }
        }
    }
}

/// The whole village, both trees and both yards included, has to fit the ground a corner can
/// spare. The widest colony is the measurement that matters; the rest are smaller by
/// construction.
#[test]
fn the_whole_village_including_both_trees_fits_the_width_it_is_allowed() {
    let span = village_span(&widest_colony());
    assert!(
        span <= VILLAGE_SPAN_LIMIT,
        "the widest village spans {span} shelter pixels"
    );
    assert!(
        village_span(&[]) < span,
        "a founder on its own should not take more ground than a village of six"
    );
    // And on a real display, at the scale that magnifies it most.
    let monitor = wide_monitor(1.0);
    let home = village_home(&monitor, HomeCorner::BottomLeft);
    let lots = placed_lots(&home, &widest_colony(), &monitor, TIGHTEST_SCALE);
    let (low, high) = lots
        .iter()
        .fold((f32::MAX, f32::MIN), |(low, high), (_, centre, half)| {
            ((centre - half).min(low), (centre + half).max(high))
        });
    let unit = f32::from(TIGHTEST_SCALE);
    assert!(
        high - low <= VILLAGE_SPAN_LIMIT * unit + 0.01,
        "the village covers {} points, more than the {} it is allowed",
        high - low,
        VILLAGE_SPAN_LIMIT * unit
    );
}

/// Where the colony settles when it is standing still. The houses stand shoulder to shoulder
/// now and the ground in front of them is shared, so a resting place is a spot on the commons
/// rather than a parcel beside one door: everybody has to be on that ground, on its line, and
/// far enough from everybody else that no face is behind another. Keeping out of the doorways
/// is a preference a roaming companion applies when there is somewhere to apply it, not a
/// promise the arrangement can keep once six houses fill the village.
#[test]
fn every_resting_companion_stands_on_the_commons_and_clear_of_the_others() {
    let policy = HabitatPolicy::default();
    let monitor = wide_monitor(1.0);
    let monitors = std::slice::from_ref(&monitor);
    let unit = f32::from(TIGHTEST_SCALE);
    for cottages in [small_colony(), widest_colony()] {
        let residents = crate::MAX_COLONY_CREATURES;
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let commons =
                home_commons(&home, &cottages, monitors, &policy, TIGHTEST_SCALE).unwrap();
            let resting: Vec<Point> = (0..residents)
                .map(|slot| {
                    home_resting_position(
                        &home,
                        slot,
                        residents,
                        &cottages,
                        monitors,
                        &policy,
                        TIGHTEST_SCALE,
                    )
                    .unwrap()
                    .1
                })
                .collect();
            let half = RESTING_WIDTH / 2.0 * unit;
            for (slot, point) in resting.iter().enumerate() {
                assert!(
                    point.x >= commons.low_x - 0.01 && point.x <= commons.high_x + 0.01,
                    "{corner:?}: companion {slot} settled off the commons"
                );
                assert!(
                    (point.y - commons.ground_y).abs() < 0.01,
                    "{corner:?}: companion {slot} left the ground line"
                );
                assert!(
                    commons.width() >= half * 2.0,
                    "{corner:?}: the commons is too narrow to stand on"
                );
            }
            let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * unit;
            for (first, one) in resting.iter().enumerate() {
                for other in resting.iter().skip(first + 1) {
                    assert!(
                        (one.x - other.x).abs() >= clear - 0.01,
                        "{corner:?}: two companions settled {} apart, closer than {clear}",
                        (one.x - other.x).abs()
                    );
                }
            }
        }
    }
}

/// Everything the colony owns lies in one of the two trees' yards, split between the ends:
/// never on a house, never on the tree itself, and never under the feet of the nearest
/// resident, who is standing out on the shared ground in front of the row.
#[test]
fn the_belongings_split_between_both_yards_without_landing_on_anything() {
    let policy = HabitatPolicy::default();
    let monitor = wide_monitor(1.0);
    let monitors = std::slice::from_ref(&monitor);
    let cottages = widest_colony();
    let unit = f32::from(TIGHTEST_SCALE);
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        let home = village_home(&monitor, corner);
        let places = home_object_positions(&home, &cottages, monitors, &policy, TIGHTEST_SCALE);
        assert_eq!(
            places.iter().flatten().count(),
            crate::MAX_COLONY_OBJECTS,
            "a display this size has room for both yards"
        );
        // Sorted into the yard each slot belongs to, so neither end carries everything.
        let mut yards: [Vec<Point>; TreeEnd::BOTH.len()] = Default::default();
        for end in TreeEnd::BOTH {
            let (_, tree) =
                home_tree_position(&home, end, &cottages, monitors, &policy, TIGHTEST_SCALE)
                    .unwrap();
            for (slot, place) in places.iter().enumerate() {
                if belonging_offset(slot, home.shelter.detail_seed).0 != end {
                    continue;
                }
                let thing = place.unwrap().1;
                assert!(
                    (thing.y - tree.y).abs() <= BELONGING_DEPTH * unit + 0.01,
                    "belonging {slot} floats {} points off the ground line",
                    (thing.y - tree.y).abs()
                );
                assert!(
                    (thing.x - tree.x).abs() + OBJECT_WIDTH / 2.0 * unit
                        <= TREE_WIDTH / 2.0 * unit + 0.01,
                    "belonging {slot} spilled out of the {end:?} yard"
                );
                yards[end.index()].push(thing);
            }
            assert_eq!(
                yards[end.index()].len(),
                crate::MAX_COLONY_OBJECTS / 2,
                "{end:?} should keep half the colony's things"
            );
        }
        // Clearance is a rule inside a yard: the two yards are a village apart.
        for (end, yard) in yards.iter().enumerate() {
            for (index, thing) in yard.iter().enumerate() {
                for (other, second) in yard.iter().enumerate().skip(index + 1) {
                    let apart = thing.distance(*second);
                    assert!(
                        apart >= BELONGING_CLEARANCE * unit - 0.01,
                        "yard {end}: belongings {index} and {other} are {apart} apart, {} \
                         needed",
                        BELONGING_CLEARANCE * unit
                    );
                }
            }
        }
        // Nobody rests in a yard. The founder is nearest the outward one, the last member
        // nearest the inward one.
        let reach = (CREATURE_FRAME_WIDTH + OBJECT_WIDTH) / 2.0 * unit;
        let residents = crate::MAX_COLONY_CREATURES;
        for slot in 0..residents {
            let (_, rest) = home_resting_position(
                &home,
                slot,
                residents,
                &cottages,
                monitors,
                &policy,
                TIGHTEST_SCALE,
            )
            .unwrap();
            for (index, thing) in places.iter().flatten().enumerate() {
                assert!(
                    (thing.1.x - rest.x).abs() >= reach,
                    "{corner:?}: member {slot} stands on belonging {index}"
                );
            }
        }
    }
    // Everything that wants the whole yard now asks for it in one go. The batch and the
    // single lookup have to keep agreeing, slot for slot.
    let home = village_home(&monitor, HomeCorner::BottomLeft);
    let places = home_object_positions(&home, &cottages, monitors, &policy, TIGHTEST_SCALE);
    for (slot, place) in places.iter().enumerate() {
        assert_eq!(
            *place,
            home_object_position(&home, slot, &cottages, monitors, &policy, TIGHTEST_SCALE),
            "belonging {slot} moved when it was asked for on its own"
        );
    }
    // The scatter comes from the colony's own seed, so it is the same every time and two
    // colonies do not arrange their yard identically.
    let one = village_home(&monitor, HomeCorner::BottomLeft);
    let mut other = one.clone();
    other.shelter.detail_seed ^= 0xffff;
    let places = |home: &ColonyHome| {
        home_object_positions(home, &cottages, monitors, &policy, TIGHTEST_SCALE)
    };
    assert_eq!(places(&one), places(&one));
    assert_ne!(places(&one), places(&other));
}

/// A tree is the first thing a cramped corner gives up. A region with room for the houses but
/// not for what stands at the ends of them shows the houses: a village with no trees in it
/// still reads as a village, and one with its colony house half off the screen does not. The
/// colony's belongings live in those yards, so they go with them.
#[test]
fn a_narrow_corner_hides_the_trees_and_their_yards_rather_than_a_house() {
    let cottages = vec![DwellingKind::Cottage];
    let mut monitor = wide_monitor(1.0);
    monitor.bounds.width = 800.0;
    monitor.usable_bounds.width = 800.0;
    let monitors = std::slice::from_ref(&monitor);
    // A habitat cut back to a band in the middle of the display: room for the houses, none for
    // what stands past them against the edge. The two houses measure 155 shelter pixels end to
    // end, so the band is 200 of them at the 2x drawn here.
    for (corner, left) in [
        (HomeCorner::BottomLeft, 0.1_f32),
        (HomeCorner::BottomRight, 0.4),
    ] {
        let policy = HabitatPolicy {
            preset: HabitatPreset::Custom,
            zones: vec![HabitatZone {
                id: 1,
                display: monitor.display_key,
                normalized_bounds: DesktopRect {
                    x: left,
                    y: 0.0,
                    width: 0.5,
                    height: 1.0,
                },
                kind: HabitatZoneKind::Allowed,
                enabled: true,
            }],
        };
        let home = village_home(&monitor, corner);
        assert!(
            home_dwelling_position(&home, 0, &cottages, monitors, &policy, 2).is_some()
                && home_dwelling_position(&home, 1, &cottages, monitors, &policy, 2).is_some(),
            "{corner:?}: a house went missing"
        );
        for end in TreeEnd::BOTH {
            assert!(
                home_tree_position(&home, end, &cottages, monitors, &policy, 2).is_none(),
                "{corner:?}: the {end:?} tree stood where there was no room for one"
            );
        }
        for slot in 0..crate::MAX_COLONY_OBJECTS {
            assert!(
                home_object_position(&home, slot, &cottages, monitors, &policy, 2).is_none(),
                "{corner:?}: belonging {slot} was left standing in a yard that is not there"
            );
        }
    }
    // Given the room, both are back: one on the far side of the house from the cottages, one
    // past the last cottage, and each inside the display it belongs to.
    let policy = HabitatPolicy::default();
    let monitor = wide_monitor(1.0);
    let monitors = std::slice::from_ref(&monitor);
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        let home = village_home(&monitor, corner);
        let (_, house) = home_dwelling_position(&home, 0, &cottages, monitors, &policy, 2).unwrap();
        let (_, cottage) =
            home_dwelling_position(&home, 1, &cottages, monitors, &policy, 2).unwrap();
        let (_, outward) =
            home_tree_position(&home, TreeEnd::Outward, &cottages, monitors, &policy, 2).unwrap();
        let (_, inward) =
            home_tree_position(&home, TreeEnd::Inward, &cottages, monitors, &policy, 2).unwrap();
        assert!(
            (outward.x - house.x).signum() != (cottage.x - house.x).signum(),
            "{corner:?}: the outward tree stood on the cottages' side of the house"
        );
        assert_eq!(
            (inward.x - house.x).signum(),
            (cottage.x - house.x).signum(),
            "{corner:?}: the inward tree left the cottages' end"
        );
        for (end, tree) in [("outward", outward), ("inward", inward)] {
            assert!(
                tree.x - TREE_WIDTH >= monitor.usable_bounds.x
                    && tree.x + TREE_WIDTH <= monitor.usable_bounds.right(),
                "{corner:?}: the {end} tree was clipped by the edge of the display"
            );
        }
    }
}

/// A corner too narrow for the whole village still seats everybody. The commons is whatever
/// ground the village did manage to lay out, and the colony spreads along it — packed in
/// against the ends if it has to, but never standing on one another.
#[test]
fn a_narrow_strip_still_seats_everybody_without_stacking_them() {
    let policy = HabitatPolicy::default();
    let mut monitor = wide_monitor(1.0);
    monitor.bounds.width = 320.0;
    monitor.usable_bounds.width = 320.0;
    let monitors = std::slice::from_ref(&monitor);
    let cottages = widest_colony();
    let residents = crate::MAX_COLONY_CREATURES;
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        let home = village_home(&monitor, corner);
        let region = accessible_regions(&policy, &monitor)[0];
        let commons = home_commons(&home, &cottages, monitors, &policy, 2).unwrap();
        assert!(
            commons.width() < VILLAGE_SPAN_LIMIT * 2.0,
            "{corner:?}: this display should be too narrow for the whole village"
        );
        let spots: Vec<f32> = (0..residents)
            .map(|slot| {
                home_resting_position(&home, slot, residents, &cottages, monitors, &policy, 2)
                    .unwrap()
                    .1
                    .x
            })
            .collect();
        for (index, a) in spots.iter().enumerate() {
            assert!(
                *a >= region.x - 0.01 && *a <= region.right() + 0.01,
                "{corner:?}: a companion was sent off the display"
            );
            for b in &spots[index + 1..] {
                assert!(
                    (a - b).abs() >= 1.0,
                    "{corner:?}: companions stacked {} apart",
                    (a - b).abs()
                );
            }
        }
    }
}

/// never within a face's width of anybody resting.
#[test]
fn a_guest_waits_past_every_lot_and_every_resident() {
    let policy = HabitatPolicy::default();
    let monitor = wide_monitor(1.0);
    let monitors = std::slice::from_ref(&monitor);
    let cottages = widest_colony();
    let unit = f32::from(TIGHTEST_SCALE);
    for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
        let home = village_home(&monitor, corner);
        let direction = if corner == HomeCorner::BottomLeft {
            1.0
        } else {
            -1.0
        };
        let anchor =
            resolved_home_anchor(&home, &cottages, &monitor, TIGHTEST_SCALE, &policy).unwrap();
        let (_, guest) =
            home_guest_position(&home, &cottages, monitors, &policy, TIGHTEST_SCALE).unwrap();
        assert_eq!(guest.y, anchor.y);
        let out = (guest.x - anchor.x) * direction;
        for (lot, centre) in village_walk(&cottages, VillageFit::Comfortable).iter() {
            let edge = (centre + lot_width(lot, &cottages, VillageFit::Comfortable) / 2.0) * unit;
            assert!(
                out >= edge - 0.01,
                "{corner:?}: the guest stands on {lot:?}"
            );
        }
        let clear = CREATURE_FRAME_WIDTH * REST_CLEAR_RATIO * unit;
        let residents = crate::MAX_COLONY_CREATURES;
        for slot in 0..residents {
            let (_, rest) = home_resting_position(
                &home,
                slot,
                residents,
                &cottages,
                monitors,
                &policy,
                TIGHTEST_SCALE,
            )
            .unwrap();
            assert!(
                (guest.x - rest.x).abs() >= clear - 0.01,
                "{corner:?}: the guest stands on member {slot}"
            );
        }
    }
}

/// The village ground works its own anchor out rather than calling `resolved_home_anchor` and
/// then listing the habitat's regions all over again, and the colony's houses are counted onto
/// the stack rather than sorted into a pair of fresh lists. Neither may change an answer.
#[test]
fn the_cheaper_layout_gives_the_same_answers_the_allocating_one_did() {
    for preset in [
        HabitatPreset::EntireDesktop,
        HabitatPreset::BottomCorners,
        HabitatPreset::BottomEdge,
    ] {
        let policy = HabitatPolicy {
            preset,
            zones: Vec::new(),
        };
        let monitor = wide_monitor(2.0);
        for corner in [HomeCorner::BottomLeft, HomeCorner::BottomRight] {
            let home = village_home(&monitor, corner);
            let ground = VillageGround::resolve(
                &home,
                &widest_colony(),
                std::slice::from_ref(&monitor),
                &policy,
                TIGHTEST_SCALE,
            )
            .unwrap();
            // The corner clamped into the nearest region, worked out the long way round, for
            // whichever fit the ground settled on: a corner region a quarter of the display
            // wide lays a full village out snug.
            let desired = anchor_for(&home, &monitor, TIGHTEST_SCALE, ground.fit);
            let expected = accessible_regions(&policy, &monitor)
                .into_iter()
                .map(|region| {
                    let point = Point {
                        x: desired.x.clamp(region.x + 8.0, region.right() - 8.0),
                        y: region.bottom() - 4.0,
                    };
                    (desired.distance(point), point)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, point)| point);
            assert_eq!(
                Some(ground.anchor),
                expected,
                "{preset:?} {corner:?}: the ground drifted from the habitat's own answer"
            );
            assert_eq!(
                ground.fit == VillageFit::Snug,
                preset == HabitatPreset::BottomCorners,
                "{preset:?} {corner:?}"
            );
            assert_eq!(
                ground.region(),
                accessible_regions(&policy, &monitor)
                    .into_iter()
                    .find(|region| {
                        ground.anchor.x >= region.x
                            && ground.anchor.x <= region.right()
                            && ground.anchor.y >= region.y
                            && ground.anchor.y <= region.bottom()
                    }),
                "{preset:?} {corner:?}: the ground kept the wrong region"
            );
        }
    }
    // A colony listed out of order, with a mini in the middle of it.
    let mut creatures = Vec::new();
    for index in 0..4_u8 {
        let mut creature = crate::World::preview_adult(
            [index; 32],
            time::OffsetDateTime::UNIX_EPOCH,
            &crate::DesktopSnapshot::default(),
        );
        creature.colony_order = 3 - index;
        creature.id = u64::from(index) + 10;
        if index == 1 {
            creature.role = CreatureRole::Mini { parent_id: 1 };
        }
        creatures.push(creature);
    }
    assert_eq!(
        colony_cottage_list(&creatures).as_slice(),
        colony_cottages(&creatures).as_slice()
    );
    // Three full-size companions and one mini: the mini lives in its big version's house,
    // so the village lays out two cottages beside the colony house rather than three.
    assert_eq!(
        colony_cottages(&creatures),
        vec![DwellingKind::Cottage, DwellingKind::Cottage],
        "the houses follow colony order, not the order the save happens to hold"
    );
}

fn monitor() -> MonitorInfo {
    MonitorInfo {
        id: 1,
        display_key: DisplayKey([1; 16]),
        bounds: DesktopRect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        },
        usable_bounds: DesktopRect {
            x: 0.0,
            y: 20.0,
            width: 1000.0,
            height: 740.0,
        },
        scale_factor: 2.0,
        primary: true,
    }
}

#[test]
fn exclusion_is_subtracted_from_allowed_space() {
    let mut policy = HabitatPolicy::default();
    policy.zones.push(HabitatZone {
        id: 1,
        display: DisplayKey([1; 16]),
        normalized_bounds: DesktopRect {
            x: 0.4,
            y: 0.0,
            width: 0.2,
            height: 1.0,
        },
        kind: HabitatZoneKind::Excluded,
        enabled: true,
    });
    let regions = accessible_regions(&policy, &monitor());
    assert_eq!(regions.len(), 2);
    assert!(
        !regions
            .iter()
            .any(|region| region.contains(Point { x: 500.0, y: 400.0 }))
    );
}

/// A display can sit left of and above the origin, and at any scale factor. Zones are stored
/// as a fraction of the display they were drawn on, so both have to come back as ordinary
/// rectangles in desktop points, and a zone drawn on a display that is no longer plugged in
/// must not take away the ground on the one that is.
#[test]
fn a_negative_origin_display_keeps_its_own_zones_and_a_lost_one_leaves_room_to_stand() {
    let far = |scale: f32| MonitorInfo {
        id: 2,
        display_key: DisplayKey([2; 16]),
        bounds: DesktopRect {
            x: -1_000.0,
            y: -600.0,
            width: 1_000.0,
            height: 600.0,
        },
        usable_bounds: DesktopRect {
            x: -1_000.0,
            y: -580.0,
            width: 1_000.0,
            height: 580.0,
        },
        scale_factor: scale,
        primary: false,
    };
    let mut policy = HabitatPolicy::default();
    policy.zones.push(HabitatZone {
        id: 4,
        display: DisplayKey([2; 16]),
        normalized_bounds: DesktopRect {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        },
        kind: HabitatZoneKind::Allowed,
        enabled: true,
    });
    let regions = accessible_regions(&policy, &far(1.0));
    assert_eq!(
        regions,
        vec![DesktopRect {
            x: -500.0,
            y: -580.0,
            width: 500.0,
            height: 580.0,
        }]
    );
    assert!(habitat_contains(
        &policy,
        &far(1.0),
        Point {
            x: -250.0,
            y: -300.0
        }
    ));
    assert!(!habitat_contains(
        &policy,
        &far(1.0),
        Point {
            x: -750.0,
            y: -300.0
        }
    ));
    assert_eq!(
        accessible_regions(&policy, &far(3.0)),
        regions,
        "a zone is a fraction of a display, not of its pixels"
    );
    // The zone belongs to that display alone; the display beside it keeps its whole preset.
    assert_eq!(
        accessible_regions(&policy, &monitor()),
        vec![monitor().usable_bounds]
    );

    // Standing between the two, a creature is sent to the nearer one; with the far display
    // unplugged, the zone that named it cannot leave the colony with nowhere to stand.
    let both = [monitor(), far(1.0)];
    assert_eq!(
        nearest_habitat_point(
            &policy,
            &both,
            Point {
                x: -200.0,
                y: -100.0
            },
            3,
        )
        .map(|(id, _)| id),
        Some(2)
    );
    assert_eq!(
        nearest_habitat_point(&policy, &both, Point { x: 800.0, y: 700.0 }, 3).map(|(id, _)| id),
        Some(1)
    );
    assert!(validate_habitat(&policy, &[monitor()]).is_ok());
    assert!(!accessible_regions(&policy, &monitor()).is_empty());
}

#[test]
fn primary_preset_excludes_secondary_displays() {
    let mut secondary = monitor();
    secondary.primary = false;
    let policy = HabitatPolicy {
        preset: HabitatPreset::PrimaryDisplay,
        zones: Vec::new(),
    };
    assert!(accessible_regions(&policy, &secondary).is_empty());
}
