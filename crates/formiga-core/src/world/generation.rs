use super::*;

pub(super) fn generated_adult(
    source_seed: [u8; 32],
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
    colony_order: u8,
    existing_names: &[String],
    kept: bool,
) -> Creature {
    let streams = SeedStream::new(source_seed);
    let mut creature =
        generate_new_creature(&streams, source_seed, 0, now, desktop, existing_names, None);
    creature.generation = 0;
    creature.colony_order = colony_order;
    creature.role = CreatureRole::Adult;
    creature.kept = kept;
    creature.mini_arrivals = MiniArrivalState {
        enabled: true,
        arrived: [false; 2],
    };
    creature
}

pub(super) fn generate_mini_for_parent(
    creatures: &[Creature],
    colony_seed: [u8; 32],
    parent_id: CreatureId,
    child_number: u8,
    now: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> Option<Creature> {
    let parent = creatures.iter().find(|creature| creature.id == parent_id)?;
    if !parent.role.is_adult() || mini_count_for_parent(creatures, parent_id) >= MAX_MINIS_PER_ADULT
    {
        return None;
    }
    let existing_names: Vec<_> = creatures
        .iter()
        .map(|creature| creature.name.clone())
        .collect();
    let imported_root = parent.generation == 0 && parent.origin.source_generation > 0;
    let generation = if imported_root {
        child_number.clamp(1, 3)
    } else {
        parent
            .origin
            .source_generation
            .saturating_add(child_number)
            .clamp(1, 3)
    };
    let mut mini = if imported_root {
        // A parent without a recipe is one of the original companions, and its mini is too.
        let shared = SharedCreatureSeed {
            source_colony_seed: colony_seed,
            source_generation: generation,
            design: parent.appearance.design.map(|design| {
                crate::CreatureDesign::generated(colony_seed, generation, Some(design))
            }),
        };
        let mut creature = generate_source_creature(shared, now, desktop);
        creature.name = default_creature_name(colony_seed, generation, &existing_names);
        creature
    } else {
        let streams = SeedStream::new(parent.origin.source_colony_seed);
        generate_new_creature(
            &streams,
            parent.origin.source_colony_seed,
            generation,
            now,
            desktop,
            &existing_names,
            Some(parent),
        )
    };
    mini.colony_order = next_colony_order(creatures);
    mini.role = CreatureRole::Mini { parent_id };
    mini.kept = true;
    mini.mini_arrivals = MiniArrivalState::default();
    Some(mini)
}

pub(super) fn generate_new_creature(
    streams: &SeedStream,
    colony_seed: [u8; 32],
    generation: u8,
    born_at_utc: OffsetDateTime,
    desktop: &DesktopSnapshot,
    existing_names: &[String],
    parent: Option<&Creature>,
) -> Creature {
    let mut creature = generate_creature(
        streams,
        colony_seed,
        generation,
        born_at_utc,
        desktop,
        existing_names,
        parent,
    );
    // An original companion has no recipe to pass on, so its mini keeps the genes it inherited
    // from it rather than taking an unrelated recipe of its own.
    if parent.is_some_and(|parent| parent.appearance.design.is_none()) {
        return creature;
    }
    let design = CreatureDesign::generated(
        colony_seed,
        generation,
        parent.and_then(|p| p.appearance.design),
    );
    apply_creature_design(&mut creature, Some(design));
    creature
}

fn generate_creature(
    streams: &SeedStream,
    colony_seed: [u8; 32],
    generation: u8,
    born_at_utc: OffsetDateTime,
    desktop: &DesktopSnapshot,
    existing_names: &[String],
    parent: Option<&Creature>,
) -> Creature {
    let mut appearance_rng = streams.rng("appearance", generation as u64);
    let mut personality_rng = streams.rng("personality", generation as u64);
    let family =
        parent
            .map(|value| value.appearance.family)
            .unwrap_or_else(|| match appearance_rng.random_range(0..3) {
                0 => BodyFamily::Blob,
                1 => BodyFamily::Hopper,
                _ => BodyFamily::SoftQuadruped,
            });
    let scale_percent = [100, 70, 62, 55][generation.min(3) as usize];
    let face_signature = parent
        .map(|value| value.appearance.face_signature)
        .unwrap_or_else(|| appearance_rng.random());
    let palette_index = parent
        .map(|value| value.appearance.palette_index)
        .unwrap_or_else(|| appearance_rng.random_range(0..12));
    let appearance = AppearanceGenome {
        design: None,
        family,
        logical_size: ((appearance_rng.random_range(34..=40) as f32) * scale_percent as f32 / 100.0)
            .round() as u8,
        body_width: mutate_parent(
            parent.map(|p| p.appearance.body_width),
            &mut appearance_rng,
            16,
            27,
        ),
        body_height: mutate_parent(
            parent.map(|p| p.appearance.body_height),
            &mut appearance_rng,
            13,
            24,
        ),
        head_ratio: mutate_float(
            parent.map(|p| p.appearance.head_ratio),
            &mut appearance_rng,
            0.55,
            1.05,
        ),
        roundness: mutate_float(
            parent.map(|p| p.appearance.roundness),
            &mut appearance_rng,
            0.35,
            1.0,
        ),
        leg_length: mutate_parent(
            parent.map(|p| p.appearance.leg_length),
            &mut appearance_rng,
            2,
            8,
        ),
        foot_size: mutate_parent(
            parent.map(|p| p.appearance.foot_size),
            &mut appearance_rng,
            2,
            5,
        ),
        head_appendages: HeadAppendageGenome {
            style: if let Some(parent) = parent {
                if appearance_rng.random_bool(0.75) {
                    parent.appearance.head_appendages.style
                } else {
                    random_head_appendage(&mut appearance_rng)
                }
            } else {
                random_head_appendage(&mut appearance_rng)
            },
            size: mutate_parent(
                parent.map(|p| p.appearance.head_appendages.size),
                &mut appearance_rng,
                2,
                7,
            ),
        },
        tail_style: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.75) {
                parent.appearance.tail_style
            } else {
                random_tail(&mut appearance_rng)
            }
        } else {
            random_tail(&mut appearance_rng)
        },
        tail_length: mutate_parent(
            parent.map(|p| p.appearance.tail_length),
            &mut appearance_rng,
            2,
            10,
        ),
        face: generate_face(parent.map(|p| p.appearance.face), &mut appearance_rng),
        forelimbs: generate_forelimbs(
            family,
            parent.map(|p| p.appearance.forelimbs),
            &mut appearance_rng,
        ),
        effect_motif: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.65) {
                parent.appearance.effect_motif
            } else {
                random_effect_motif(&mut appearance_rng)
            }
        } else {
            random_effect_motif(&mut appearance_rng)
        },
        palette_index,
        pattern: if let Some(parent) = parent {
            if appearance_rng.random_bool(0.6) {
                parent.appearance.pattern
            } else {
                random_pattern(&mut appearance_rng)
            }
        } else {
            random_pattern(&mut appearance_rng)
        },
        pattern_density: mutate_float(
            parent.map(|p| p.appearance.pattern_density),
            &mut appearance_rng,
            0.12,
            0.75,
        ),
        marking_seed: appearance_rng.random(),
        gait_bob: mutate_float(
            parent.map(|p| p.appearance.gait_bob),
            &mut appearance_rng,
            0.15,
            0.9,
        ),
        face_signature,
    };
    let personality = PersonalityGenome {
        activity: personality_rng.random_range(0.2..0.95),
        curiosity: personality_rng.random_range(0.15..0.95),
        boldness: personality_rng.random_range(0.1..0.95),
        playfulness: personality_rng.random_range(0.15..0.95),
        sociability: personality_rng.random_range(0.25..0.95),
        routine_affinity: personality_rng.random_range(0.1..0.9),
        sleep_timing: personality_rng.random_range(0.2..0.9),
        window_tolerance: personality_rng.random_range(0.1..0.95),
        cursor_interest: personality_rng.random_range(0.1..0.95),
        decision_temperature: personality_rng.random_range(0.22..0.75),
    };
    let monitor = desktop
        .monitors
        .iter()
        .find(|monitor| monitor.primary)
        .or_else(|| desktop.monitors.first());
    let (position, monitor_id) = monitor
        .map(|monitor| {
            (
                Point {
                    x: monitor.usable_bounds.x
                        + monitor.usable_bounds.width * 0.35
                        + generation as f32 * 42.0,
                    y: monitor.usable_bounds.bottom() - 4.0,
                },
                monitor.id,
            )
        })
        .unwrap_or((Point { x: 320.0, y: 700.0 }, 0));
    Creature {
        id: u64::from_le_bytes(
            streams.bytes("creature-id", generation as u64)[..8]
                .try_into()
                .unwrap(),
        ),
        generation,
        origin: CreatureOrigin {
            design: None,
            source_colony_seed: colony_seed,
            source_generation: generation,
        },
        colony_order: generation,
        role: CreatureRole::Adult,
        kept: true,
        mini_arrivals: MiniArrivalState::default(),
        name: default_creature_name(colony_seed, generation, existing_names),
        born_at_utc,
        display_scale_percent: scale_percent,
        appearance,
        personality,
        behavior_seed: streams.bytes("behavior", generation as u64),
        memory: CreatureMemory::default(),
        tendencies: LearnedTendencies::default(),
        routines: RoutineTable::default(),
        leaning: RoamingLeaning::default(),
        accessory: None,
        state: CreatureState {
            attention: None,
            position,
            velocity: Point::default(),
            facing_right: true,
            action: ActionKind::Idle,
            action_elapsed: 0.0,
            action_duration: 2.5,
            drives: Drives::default(),
            surface: SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id,
                window_key: None,
                relative_x: 0.35,
            },
            cursor_cooldown: 0.0,
            activity_variant: 0,
            arrival_delay_secs: 0.0,
            flourish: None,
            nudge: None,
            beat: None,
            indoors: false,
        },
    }
}

pub(super) fn generate_source_creature(
    shared: SharedCreatureSeed,
    born_at_utc: OffsetDateTime,
    desktop: &DesktopSnapshot,
) -> Creature {
    let streams = SeedStream::new(shared.source_colony_seed);
    let mut creatures = Vec::with_capacity(usize::from(shared.source_generation) + 1);
    for generation in 0..=shared.source_generation.min(3) {
        let existing_names: Vec<_> = creatures
            .iter()
            .map(|creature: &Creature| creature.name.clone())
            .collect();
        let creature = generate_creature(
            &streams,
            shared.source_colony_seed,
            generation,
            born_at_utc,
            desktop,
            &existing_names,
            creatures.first(),
        );
        creatures.push(creature);
    }
    let mut creature = creatures
        .pop()
        .expect("a source generation is always built");
    apply_creature_design(&mut creature, shared.design);
    creature
}

fn mutate_parent<R: Rng + ?Sized>(parent: Option<u8>, rng: &mut R, min: u8, max: u8) -> u8 {
    parent
        .map(|value| (value as i16 + rng.random_range(-2..=2)).clamp(min as i16, max as i16) as u8)
        .unwrap_or_else(|| rng.random_range(min..=max))
}

fn mutate_float<R: Rng + ?Sized>(parent: Option<f32>, rng: &mut R, min: f32, max: f32) -> f32 {
    parent
        .map(|value| (value + rng.random_range(-0.12..0.12)).clamp(min, max))
        .unwrap_or_else(|| rng.random_range(min..max))
}

fn random_head_appendage<R: Rng + ?Sized>(rng: &mut R) -> HeadAppendageStyle {
    match rng.random_range(0..6) {
        0 => HeadAppendageStyle::None,
        1 => HeadAppendageStyle::Round,
        2 => HeadAppendageStyle::Pointed,
        3 => HeadAppendageStyle::Leaf,
        4 => HeadAppendageStyle::Droop,
        _ => HeadAppendageStyle::Antenna,
    }
}

fn generate_face<R: Rng + ?Sized>(parent: Option<FaceGenome>, rng: &mut R) -> FaceGenome {
    if let Some(parent) = parent {
        return FaceGenome {
            // These genes form the inherited face signature shared by the colony.
            eye_shape: parent.eye_shape,
            eye_size: parent.eye_size,
            eye_spacing: parent.eye_spacing,
            vertical_offset: parent.vertical_offset,
            pupil_style: parent.pupil_style,
            highlight_style: parent.highlight_style,
            brow_style: if rng.random_bool(0.28) {
                random_brow(rng)
            } else {
                parent.brow_style
            },
            mouth_style: if rng.random_bool(0.2) {
                random_mouth(rng)
            } else {
                parent.mouth_style
            },
            cheek_style: if rng.random_bool(0.35) {
                random_cheek(rng)
            } else {
                parent.cheek_style
            },
        };
    }
    FaceGenome {
        eye_shape: match rng.random_range(0..3) {
            0 => EyeShape::Round,
            1 => EyeShape::Tall,
            _ => EyeShape::SoftSquare,
        },
        eye_size: rng.random_range(1..=2),
        eye_spacing: rng.random_range(4..=7),
        vertical_offset: rng.random_range(-1..=1),
        pupil_style: match rng.random_range(0..3) {
            0 => PupilStyle::Dot,
            1 => PupilStyle::Wide,
            _ => PupilStyle::Spark,
        },
        highlight_style: match rng.random_range(0..3) {
            0 => HighlightStyle::Single,
            1 => HighlightStyle::Double,
            _ => HighlightStyle::Diagonal,
        },
        brow_style: random_brow(rng),
        mouth_style: random_mouth(rng),
        cheek_style: random_cheek(rng),
    }
}

fn generate_forelimbs<R: Rng + ?Sized>(
    family: BodyFamily,
    parent: Option<ForelimbGenome>,
    rng: &mut R,
) -> ForelimbGenome {
    let (style, tip_style) = match family {
        BodyFamily::Blob => (
            if rng.random_bool(0.5) {
                ForelimbStyle::SoftNub
            } else {
                ForelimbStyle::Pseudopod
            },
            LimbTipStyle::Round,
        ),
        BodyFamily::Hopper => (ForelimbStyle::MittenArm, LimbTipStyle::Mitten),
        BodyFamily::SoftQuadruped => (ForelimbStyle::FrontPaw, LimbTipStyle::Paw),
    };
    ForelimbGenome {
        style: parent.map_or(style, |value| value.style),
        length: mutate_parent(parent.map(|value| value.length), rng, 3, 7),
        thickness: mutate_parent(parent.map(|value| value.thickness), rng, 1, 2),
        tip_style: parent.map_or(tip_style, |value| value.tip_style),
        rest_pose: if let Some(parent) = parent {
            if rng.random_bool(0.3) {
                random_rest_pose(rng)
            } else {
                parent.rest_pose
            }
        } else {
            random_rest_pose(rng)
        },
    }
}

fn random_brow<R: Rng + ?Sized>(rng: &mut R) -> BrowStyle {
    match rng.random_range(0..3) {
        0 => BrowStyle::None,
        1 => BrowStyle::Soft,
        _ => BrowStyle::Bold,
    }
}

fn random_mouth<R: Rng + ?Sized>(rng: &mut R) -> MouthStyle {
    match rng.random_range(0..4) {
        0 => MouthStyle::Tiny,
        1 => MouthStyle::Smile,
        2 => MouthStyle::Cat,
        _ => MouthStyle::Beak,
    }
}

fn random_cheek<R: Rng + ?Sized>(rng: &mut R) -> CheekStyle {
    match rng.random_range(0..3) {
        0 => CheekStyle::None,
        1 => CheekStyle::Dots,
        _ => CheekStyle::Blush,
    }
}

fn random_rest_pose<R: Rng + ?Sized>(rng: &mut R) -> RestPose {
    match rng.random_range(0..3) {
        0 => RestPose::AtSides,
        1 => RestPose::Folded,
        _ => RestPose::Together,
    }
}

fn random_effect_motif<R: Rng + ?Sized>(rng: &mut R) -> EffectMotif {
    match rng.random_range(0..6) {
        0 => EffectMotif::None,
        1 => EffectMotif::Dot,
        2 => EffectMotif::Star,
        3 => EffectMotif::Heart,
        4 => EffectMotif::Leaf,
        _ => EffectMotif::Spark,
    }
}

fn random_tail<R: Rng + ?Sized>(rng: &mut R) -> TailStyle {
    match rng.random_range(0..5) {
        0 => TailStyle::None,
        1 => TailStyle::Stub,
        2 => TailStyle::Taper,
        3 => TailStyle::Tuft,
        _ => TailStyle::Curl,
    }
}

fn random_pattern<R: Rng + ?Sized>(rng: &mut R) -> PatternKind {
    match rng.random_range(0..7) {
        0 => PatternKind::Solid,
        1 => PatternKind::Patches,
        2 => PatternKind::Spots,
        3 => PatternKind::Stripes,
        4 => PatternKind::Mask,
        5 => PatternKind::Socks,
        _ => PatternKind::Tips,
    }
}
