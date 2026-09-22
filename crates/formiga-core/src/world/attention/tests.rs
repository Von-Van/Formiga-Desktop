use super::*;
use time::macros::datetime;

pub(super) fn scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let created = datetime!(2026-01-01 0:00 UTC);
    let now = created + Duration::days(40);
    let mut desktop = super::super::tests::desktop();
    desktop.window_sample = Some(WindowSample {
        monotonic_millis: 0,
        reliable: true,
    });
    desktop.windows.push(DesktopWindow {
        key: 701,
        bounds: DesktopRect {
            x: 200.0,
            y: 600.0,
            width: 600.0,
            height: 200.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let mut world = World::new([92; 32], created, &desktop);
    world.tick(now, 0.05, &desktop);
    super::super::tests::let_colony_wander(&mut world, now);
    world.save.ritual.next_at_utc = now + Duration::days(1);
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.arrival_delay_secs = 0.0;
        creature.state.action = ActionKind::Idle;
        creature.state.action_elapsed = 0.0;
        creature.state.action_duration = 100.0;
        creature.state.drives = Drives::default();
        creature.personality.curiosity = 1.0;
        creature.personality.sociability = 1.0;
        creature.personality.boldness = if index == 1 { 1.0 } else { 0.0 };
        creature.personality.window_tolerance = if index == 1 { 1.0 } else { 0.0 };
        if index == 1 {
            creature.personality.curiosity = 0.6;
        }
        if index < 2 {
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::WindowLedge,
                monitor_id: 1,
                window_key: Some(701),
                relative_x: 0.2 + index as f32 * 0.4,
            };
            creature.state.position = Point {
                x: 320.0 + index as f32 * 240.0,
                y: 600.0,
            };
        } else {
            creature.state.surface = SurfaceAttachment {
                kind: SurfaceKind::ScreenFloor,
                monitor_id: 1,
                window_key: None,
                relative_x: 0.5,
            };
            creature.state.position = Point {
                x: 600.0 + (index - 2) as f32 * 100.0,
                y: 846.0,
            };
        }
    }
    world.tick(now, 0.05, &desktop);
    world.drain_events().for_each(drop);
    (world, desktop, now)
}

/// Fills a fixture's colony to the cap with full-size companions, all of them already arrived,
/// so a test can put the fifth and sixth wherever it needs them.
pub(super) fn fill_colony(world: &mut World, desktop: &DesktopSnapshot, now: OffsetDateTime) {
    let mut seed = 160_u8;
    while world.save.creatures.len() < MAX_COLONY_CREATURES {
        seed += 1;
        world
            .add_designed_adult([seed; 32], None, now, desktop)
            .unwrap();
    }
    for creature in &mut world.save.creatures {
        creature.state.arrival_delay_secs = 0.0;
    }
}

fn start(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime) {
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
    desktop.windows[0].bounds.x += 120.0;
    world.tick(now + Duration::milliseconds(250), 0.05, desktop);
}

/// A window opening above the floor the colony is standing on: the plainest window-watching
/// scene there is, and the one the whole vocabulary of window attention exists for. Play it
/// from step 21, once the opening is the only piece of news on the desktop.
pub(super) fn arrival_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut world, mut desktop, now) = scene();
    for step in 1..=20 {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            &desktop,
        );
    }
    world.clear_attention();
    desktop.windows.push(DesktopWindow {
        key: 777,
        bounds: DesktopRect {
            x: 840.0,
            y: 500.0,
            width: 320.0,
            height: 300.0,
        },
        z_order: 5,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    (world, desktop, now)
}

/// Actions whose own clip is the whole point of them. A pose never stands in for one.
const BODY_OWNING: [ActionKind; 16] = [
    ActionKind::Traverse,
    ActionKind::Sprint,
    ActionKind::Follow,
    ActionKind::SqueezeWindow,
    ActionKind::Landing,
    ActionKind::Dragged,
    ActionKind::Tossed,
    ActionKind::Dangle,
    ActionKind::ClimbWindow,
    ActionKind::Sleep,
    ActionKind::RideWindow,
    ActionKind::Eat,
    ActionKind::Drink,
    ActionKind::Homebound,
    ActionKind::AvoidCursor,
    ActionKind::PresentDiscovery,
];

/// Every body pose a run of ticks left on screen, checked tick by tick against the rules for
/// when one may replace the action's own clip: never with reduced motion, never while
/// walking, hopping, hanging, tossed, or carried by a journey, only over a planted
/// presentation, and a reach always toward what the creature is looking at. Scenes read their
/// poses through this, so "the colony struck a pose" means the same thing in every test.
#[derive(Default, Debug)]
pub(super) struct Poses {
    /// Each creature's poses in the order it struck them, with unbroken repeats collapsed.
    pub(super) struck: BTreeMap<CreatureId, Vec<Gesture>>,
    /// How many ticks each creature spent showing each pose.
    pub(super) held: BTreeMap<CreatureId, Vec<(Gesture, usize)>>,
    last: BTreeMap<CreatureId, Gesture>,
}

impl Poses {
    /// Run one tick, then check and write down every pose it left on screen.
    pub(super) fn tick(&mut self, world: &mut World, tick: impl FnOnce(&mut World)) {
        let before: Vec<(CreatureId, Point)> = world
            .save
            .creatures
            .iter()
            .map(|c| (c.id, c.state.position))
            .collect();
        tick(world);
        self.note(world, &before);
    }

    fn note(&mut self, world: &World, before: &[(CreatureId, Point)]) {
        for c in &world.save.creatures {
            let shown = c.state.attention.and_then(|pose| pose.gesture);
            let Some(gesture) = shown else {
                self.last.remove(&c.id);
                continue;
            };
            let pose = c.state.attention.unwrap();
            let label = format!(
                "{gesture:?} over {:?} by creature {} at {:?}",
                c.state.action, c.id, c.state.position
            );
            assert!(
                !world.save.settings.reduce_motion,
                "reduced motion: {label}"
            );
            assert!(!BODY_OWNING.contains(&c.state.action), "{label}");
            assert!(
                matches!(
                    c.state.action,
                    ActionKind::Idle
                        | ActionKind::Perch
                        | ActionKind::InspectScreen
                        | ActionKind::Greet
                        | ActionKind::SocialPlay
                        | ActionKind::SoloPlay
                        | ActionKind::ReactToWindow
                        | ActionKind::InvestigateCursor
                ),
                "not a planted presentation: {label}"
            );
            assert_eq!(pose.hanging, 0.0, "hanging: {label}");
            assert!(!world.tosses.contains_key(&c.id), "tossed: {label}");
            let plan = world
                .attention
                .plans
                .get(&c.id)
                .unwrap_or_else(|| panic!("a pose outlived its scene: {label}"));
            assert!(
                plan.walk.is_none() && plan.display_walk.is_none(),
                "walking: {label}"
            );
            assert!(
                !matches!(plan.role, Role::Play { hopping: true, .. }),
                "hopping: {label}"
            );
            if world.window_journeys.contains_key(&c.id) {
                let was = before.iter().find(|(id, _)| *id == c.id).map(|(_, at)| *at);
                assert_eq!(was, Some(c.state.position), "carried: {label}");
            }
            if gesture == Gesture::Reach && (pose.target.x - c.state.position.x).abs() > 1.0 {
                assert_eq!(
                    c.state.facing_right,
                    pose.target.x > c.state.position.x,
                    "reaching away from {:?}: {label}",
                    pose.target
                );
            }
            // A watching pose is a claim about where a creature's attention is. A creature
            // holding it while turned the other way is telling the plainest possible lie, so
            // the recorder checks it wherever the target is far enough off to have a side.
            if gesture == Gesture::Watch && (pose.target.x - c.state.position.x).abs() > 8.0 {
                assert_eq!(
                    c.state.facing_right,
                    pose.target.x > c.state.position.x,
                    "watching {:?} over its shoulder: {label}",
                    pose.target
                );
            }
            if self.last.insert(c.id, gesture) != Some(gesture) {
                self.struck.entry(c.id).or_default().push(gesture);
            }
            let held = self.held.entry(c.id).or_default();
            match held.iter_mut().find(|(g, _)| *g == gesture) {
                Some((_, ticks)) => *ticks += 1,
                None => held.push((gesture, 1)),
            }
        }
    }

    /// Whether anyone struck this pose.
    pub(super) fn showed(&self, gesture: Gesture) -> bool {
        self.struck.values().any(|poses| poses.contains(&gesture))
    }

    /// The poses one creature struck, in order.
    pub(super) fn by(&self, id: CreatureId) -> &[Gesture] {
        self.struck.get(&id).map_or(&[], Vec::as_slice)
    }

    /// How many ticks one creature held one pose, all told.
    pub(super) fn ticks(&self, id: CreatureId, gesture: Gesture) -> usize {
        self.held
            .get(&id)
            .and_then(|held| held.iter().find(|(g, _)| *g == gesture))
            .map_or(0, |(_, ticks)| *ticks)
    }

    /// Every pose anyone struck.
    pub(super) fn all(&self) -> Vec<Gesture> {
        let mut all = Vec::new();
        for gesture in self.struck.values().flatten() {
            if !all.contains(gesture) {
                all.push(*gesture);
            }
        }
        all
    }
}

/// What a run of scenes asked of the creatures' bodies, so a sweep for poses that never
/// appear where the body is busy can show that the body really was busy.
#[derive(Default, Debug)]
struct Busy {
    actions: Vec<ActionKind>,
    hung: bool,
    hopped: bool,
    tossed: bool,
}

/// Play a scene out, recording its poses and what its bodies were doing meanwhile.
fn watch(
    world: &mut World,
    desktop: &mut DesktopSnapshot,
    now: OffsetDateTime,
    steps: std::ops::Range<u64>,
    poses: &mut Poses,
    busy: &mut Busy,
) {
    for step in steps {
        play_out(world, desktop, now, step..step + 1, poses);
        for creature in &world.save.creatures {
            if BODY_OWNING.contains(&creature.state.action)
                && !busy.actions.contains(&creature.state.action)
            {
                busy.actions.push(creature.state.action);
            }
            busy.hung |= creature
                .state
                .attention
                .is_some_and(|pose| pose.hanging > 0.0);
        }
        busy.hopped |= world.attention.plans.values().any(|plan| {
            matches!(
                plan.role,
                Role::Play { hopping: true, .. } | Role::Journey { .. }
            )
        });
        busy.tossed |= !world.tosses.is_empty();
    }
}

/// Play a scene out tick by tick, writing down every pose it shows.
fn play_out(
    world: &mut World,
    desktop: &mut DesktopSnapshot,
    now: OffsetDateTime,
    steps: std::ops::Range<u64>,
    poses: &mut Poses,
) {
    for step in steps {
        poses.tick(world, |world| {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
            world.tick(
                now + Duration::milliseconds(step as i64 * 50),
                0.05,
                desktop,
            );
        });
    }
}

/// Every pose in the vocabulary, and for each of them a scene the colony plays out by itself
/// to strike it. This is what keeps the vocabulary honest: a pose nothing ever reaches is a
/// pose nobody will see. Every tick of every scene here is also checked against the rules for
/// showing one at all.
#[test]
fn every_pose_in_the_vocabulary_has_a_scene_that_strikes_it() {
    let mut poses = Poses::default();
    // A window opening on the desktop: noticed, approached, and then watched.
    let (mut world, mut desktop, now) = arrival_scene();
    play_out(&mut world, &mut desktop, now, 21..160, &mut poses);
    assert!(
        poses.showed(Gesture::Watch),
        "an arriving window was never watched: {poses:?}"
    );
    // A bold leap over a gap, in front of the colony: the jumper squares up and celebrates,
    // a timid watcher hides its eyes, a bolder one frets through it.
    let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(true, true);
    play_out(&mut world, &mut desktop, now, 3..240, &mut poses);
    // A leap that only just makes it: the colony gasps, a companion hauls it up, and the
    // jumper wobbles on the edge before it is pleased with itself.
    let (mut world, mut desktop, now) = super::ledges::tests::marginal_scene(true);
    play_out(&mut world, &mut desktop, now, 3..240, &mut poses);
    // A circle of dancers, each on its own beat.
    let (mut world, mut desktop, now) = super::games::tests::dance_scene();
    play_out(&mut world, &mut desktop, now, 1..260, &mut poses);
    // A toy nobody else can have.
    let (mut world, mut desktop, now) = super::games::tests::keep_away_scene();
    play_out(&mut world, &mut desktop, now, 1..300, &mut poses);
    for gesture in Gesture::ALL
        .into_iter()
        .filter(|gesture| gesture.in_scenes())
    {
        assert!(
            poses.showed(gesture),
            "no scene ever struck {gesture:?}; between them these showed {:?}",
            poses.all()
        );
    }
}

/// Reduced motion is a promise that nothing will move about on its own, and a body pose is
/// movement. The same scenes that are full of poses show none of them with the setting on.
#[test]
fn reduced_motion_strikes_no_pose_at_all() {
    for reduced in [false, true] {
        let mut poses = Poses::default();
        // A leap, a catch, and a companion going to help.
        let (mut world, mut desktop, now) = super::ledges::tests::marginal_scene(true);
        world.save.settings.reduce_motion = reduced;
        play_out(&mut world, &mut desktop, now, 3..200, &mut poses);
        // A copy chain, which is one of the two scenes reduced motion still allows.
        let (mut world, mut desktop, now) = super::play::tests::scene(true);
        world.save.settings.reduce_motion = reduced;
        play_out(&mut world, &mut desktop, now, 1..200, &mut poses);
        // A peek over a long drop.
        let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(false, false);
        world.save.settings.reduce_motion = reduced;
        play_out(&mut world, &mut desktop, now, 3..80, &mut poses);
        assert_eq!(
            poses.all().is_empty(),
            reduced,
            "reduced motion {reduced} showed {:?}",
            poses.all()
        );
    }
}

/// A pose only ever stands in for a body with nothing else to do. These scenes spend most of
/// their time travelling, leaping, hanging by the hands and falling; the recorder checks every
/// tick of every one of them, and the tally afterwards proves they really did put the body to
/// work rather than standing about being easy to satisfy.
#[test]
fn a_pose_never_stands_in_for_a_body_its_action_is_already_using() {
    let mut poses = Poses::default();
    let mut busy = Busy::default();
    // A marginal leap with a helper, where the attempt slips and both of them come off the
    // ledge: run-ups, flight, hanging by the hands, a rescue, and a fall.
    let (mut world, mut desktop, now) = super::ledges::tests::marginal_scene(true);
    let actor = world.save.creatures[0].id;
    for step in 3..230 {
        if let Some(WindowJourney::Gap(gap)) = world.window_journeys.get_mut(&actor) {
            gap.assistance_slip = true;
        }
        watch(
            &mut world,
            &mut desktop,
            now,
            step..step + 1,
            &mut poses,
            &mut busy,
        );
    }
    // Companions vaulting over one another the length of a ledge.
    let (mut world, mut desktop, now) = super::games::tests::leapfrog_scene();
    watch(&mut world, &mut desktop, now, 1..300, &mut poses, &mut busy);
    // A race across the desktop, which travels and leaps the whole way.
    let (mut world, mut desktop, now) = super::geometry_games::tests::race_scene(2);
    watch(
        &mut world,
        &mut desktop,
        now,
        80..700,
        &mut poses,
        &mut busy,
    );
    assert!(
        busy.actions.len() >= 5 && busy.hung && busy.hopped && busy.tossed,
        "the scenes were never busy enough to mean anything: {busy:?}"
    );
    assert!(!poses.all().is_empty(), "and nothing was ever posed at all");
}

/// A creature reacting to a window has to be looking at the window. The rectangle clamped to
/// the creature's own feet is not that: beside a window it aims at the floor by its toes, and
/// on one it hands the creature back its own position, which is a gaze with no direction in it
/// at all. Both cases are checked here against the window itself.
#[test]
fn a_window_reaction_aims_at_the_window_rather_than_at_the_creatures_own_feet() {
    let bounds = DesktopRect {
        x: 200.0,
        y: 600.0,
        width: 600.0,
        height: 200.0,
    };
    let centre = Point {
        x: bounds.x + bounds.width * 0.5,
        y: bounds.y + bounds.height * 0.5,
    };
    // Standing on the window, anywhere along it: the look goes to its middle, down and along
    // the surface underfoot, rather than straight at the creature's own toes.
    for relative in [0.05, 0.5, 0.95] {
        let standing = Point {
            x: bounds.x + bounds.width * relative,
            y: bounds.y,
        };
        assert_eq!(window_gaze(bounds, standing), centre);
        assert_ne!(window_gaze(bounds, standing), bounds.clamp(standing));
    }
    // Standing beside it: the near edge, halfway down, whatever height the creature is at.
    for (position, edge) in [(100.0, bounds.x), (900.0, bounds.right())] {
        for height in [0.0, bounds.y, 2_000.0] {
            let beside = Point {
                x: position,
                y: height,
            };
            let aim = window_gaze(bounds, beside);
            assert_eq!(aim.x, edge);
            assert_eq!(aim.y, centre.y);
        }
    }
    // And the whole of it lands on the window, which is the only claim that really matters.
    for x in [0.0, 100.0, 500.0, 900.0] {
        for y in [0.0, 600.0, 900.0] {
            assert!(bounds.contains(window_gaze(bounds, Point { x, y })));
        }
    }
}

/// A support that is simply not there any more is a fright first and a puzzle second: the
/// creature gasps where it stood, and then gets on with looking for where the window went
/// rather than wearing the gasp for the whole search.
#[test]
fn a_vanished_support_gasps_once_and_then_only_looks_around() {
    let (mut world, mut desktop, now) = scene();
    start(&mut world, &mut desktop, now);
    desktop.windows.clear();
    let mut poses = Poses::default();
    play_out(&mut world, &mut desktop, now, 11..40, &mut poses);
    let searcher = world.save.creatures[0].id;
    assert_eq!(poses.by(searcher), [Gesture::Gasp], "{poses:?}");
    assert!(
        poses.ticks(searcher, Gesture::Gasp) <= 12,
        "a gasp is a moment, not a mood: {poses:?}"
    );
    assert!(matches!(
        world.attention.plans[&searcher].role,
        Role::Actor { vanished: true, .. }
    ));
    assert_eq!(
        world.save.creatures[0]
            .state
            .attention
            .expect("still searching")
            .gesture,
        None
    );
}

/// Several windows shoved about at once is one piece of news, not one piece per window. The
/// colony notices the rearrangement, reacts to it once, and settles again; nobody is
/// interrupted afresh by each window that moved.
#[test]
fn a_flurry_of_window_moves_is_one_piece_of_news_rather_than_one_per_window() {
    let (mut world, mut desktop, now) = scene();
    // Three more windows beside the first, close enough together to read as one cluster.
    for (index, x) in [820.0, 900.0, 980.0].into_iter().enumerate() {
        let mut window = desktop.windows[0].clone();
        window.key = 710 + index as u64;
        window.bounds = DesktopRect {
            x,
            y: 560.0,
            width: 70.0,
            height: 240.0,
        };
        window.z_order = 1 + index as u32;
        desktop.windows.push(window);
    }
    let tick = |world: &mut World, desktop: &mut DesktopSnapshot, step: i64| {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step as u64 * 50;
        world.tick(now + Duration::milliseconds(step * 50), 0.05, desktop);
    };
    // Let the new windows stop being news before the flurry itself begins.
    for step in 1..=60 {
        tick(&mut world, &mut desktop, step);
    }
    world.clear_attention();
    let mut origins: Vec<Origin> = Vec::new();
    let mut busiest = 0;
    for step in 61..=260 {
        if (61..=76).contains(&step) {
            for window in desktop.windows.iter_mut().skip(1) {
                window.bounds.x += 12.0;
            }
        }
        tick(&mut world, &mut desktop, step);
        for plan in world.attention.plans.values() {
            if !origins.contains(&plan.origin) {
                origins.push(plan.origin);
            }
        }
        busiest = busiest.max(world.attention.plans.len());
    }
    assert!(!origins.is_empty(), "a rearrangement is worth noticing");
    assert!(
        origins.len() <= 2,
        "one flurry, not a scene per window that moved: {origins:?}"
    );
    assert!(
        busiest <= world.save.creatures.len(),
        "nobody is recruited twice over"
    );
    assert!(
        world.attention.plans.is_empty(),
        "and the colony settles once the desktop does"
    );
}

#[test]
fn one_window_produces_distinct_riders_and_a_delayed_audience_with_one_origin() {
    let (mut world, mut desktop, now) = scene();
    start(&mut world, &mut desktop, now);
    assert_eq!(world.attention.plans.len(), 4);
    let timid = world.save.creatures[0].id;
    let bold = world.save.creatures[1].id;
    assert_eq!(
        world.attention.plans[&timid].emotion,
        AttentionEmotion::Startled
    );
    assert_eq!(
        world.attention.plans[&bold].emotion,
        AttentionEmotion::Enjoying
    );
    for creature in &world.save.creatures[2..] {
        let plan = world.attention.plans[&creature.id];
        assert!(matches!(plan.role, Role::Observer { actor } if actor == timid));
        assert_eq!(plan.origin, world.attention.plans[&timid].origin);
        assert!(creature.state.attention.is_none());
    }
    for step in 1..=24 {
        world.tick(
            now + Duration::milliseconds(250 + step * 50),
            0.05,
            &desktop,
        );
    }
    assert_eq!(
        world.save.creatures[0].state.attention.unwrap().emotion,
        AttentionEmotion::Startled
    );
    assert_eq!(
        world.save.creatures[1].state.attention.unwrap().emotion,
        AttentionEmotion::Enjoying
    );
    for creature in &world.save.creatures[2..] {
        let pose = creature.state.attention.unwrap();
        assert_eq!(pose.emotion, AttentionEmotion::Concerned);
        assert_eq!(
            pose.target,
            head_point(&world.save.creatures[0], &world.save.settings, &desktop)
        );
    }
}

#[test]
fn continuous_motion_does_not_restart_reactions_and_the_audience_returns_to_life() {
    let (mut world, mut desktop, now) = scene();
    start(&mut world, &mut desktop, now);
    let origins: Vec<_> = world.attention.plans.values().map(|p| p.origin).collect();
    for step in 1..=90 {
        if step % 5 == 0 {
            desktop.window_sample.as_mut().unwrap().monotonic_millis = 250 + step * 50;
            desktop.windows[0].bounds.x += 8.0;
        }
        world.tick(
            now + Duration::milliseconds((250 + step * 50) as i64),
            0.05,
            &desktop,
        );
        if step == 20 {
            assert!(world.attention.plans.values().all(|p| p.elapsed >= 0.99));
            assert_eq!(
                world
                    .attention
                    .plans
                    .values()
                    .map(|p| p.origin)
                    .collect::<Vec<_>>(),
                origins
            );
        }
    }
    assert!(world.attention.plans.is_empty());
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_none())
    );
    assert!(
        world
            .attention
            .cooldowns
            .values()
            .all(|remaining| *remaining < REACTION_COOLDOWN)
    );
    assert!(!world.drain_events().any(|event| matches!(
        event,
        WorldEvent::ActionCompleted {
            action: ActionKind::InspectScreen,
            ..
        }
    )));
}

#[test]
fn interaction_cancels_actor_and_its_observers_before_the_next_tick() {
    let (mut world, mut desktop, now) = scene();
    start(&mut world, &mut desktop, now);
    let id = world.save.creatures[0].id;
    let cursor = world.save.creatures[0].state.position;
    assert!(world.handle_command(
        WorldCommand::BeginInteraction {
            creature_id: id,
            cursor
        },
        &desktop
    ));
    assert!(!world.attention.owns(id));
    assert!(world.save.creatures[0].state.attention.is_none());
    assert!(
        world.save.creatures[2..]
            .iter()
            .all(|c| !world.attention.owns(c.id))
    );
    assert!(world.handle_command(WorldCommand::CancelInteraction, &desktop));
    assert_ne!(world.save.creatures[0].state.action_duration, f32::MAX);
}

#[test]
fn pause_hide_home_and_unreliable_scans_cancel_without_a_replay() {
    for reason in 0..4 {
        let (mut world, mut desktop, now) = scene();
        start(&mut world, &mut desktop, now);
        match reason {
            0 => world.save.settings.paused = true,
            1 => world.save.settings.visible = false,
            2 => world.set_quiet_mode(15, now),
            _ => desktop.window_sample.as_mut().unwrap().reliable = false,
        }
        world.tick(now + Duration::milliseconds(300), 0.05, &desktop);
        assert!(world.attention.plans.is_empty());
        assert!(
            world
                .save
                .creatures
                .iter()
                .all(|c| c.state.attention.is_none())
        );
        world.save.settings.paused = false;
        world.save.settings.visible = true;
        world.save.companion.quiet_until = None;
        super::super::tests::let_colony_wander(&mut world, now);
        desktop.window_sample = Some(WindowSample {
            monotonic_millis: 500,
            reliable: true,
        });
        world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
        assert!(world.attention.plans.is_empty());
    }
}

#[test]
fn vanished_support_recovers_safely_and_cancels_watchers() {
    let (mut world, mut desktop, now) = scene();
    start(&mut world, &mut desktop, now);
    desktop.windows.clear();
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
    world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
    assert!(world.attention.plans.is_empty());
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_none())
    );
    for c in &world.save.creatures[..2] {
        assert_eq!(c.state.surface.kind, SurfaceKind::ScreenFloor);
        assert_eq!(c.state.action, ActionKind::ReactToWindow);
        assert!(habitat_contains(
            &world.save.settings.habitat,
            &desktop.monitors[0],
            c.state.position
        ));
    }
}

#[test]
fn poses_are_not_saved_and_reduced_motion_keeps_calm_stationary_gaze() {
    let (mut world, mut desktop, now) = scene();
    world.save.settings.reduce_motion = true;
    start(&mut world, &mut desktop, now);
    for step in 1..=24 {
        world.tick(now + Duration::milliseconds(step * 50), 0.05, &desktop);
    }
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.action == ActionKind::InspectScreen)
    );
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_some())
    );
    let json = serde_json::to_string(&world.save).unwrap();
    assert!(!json.contains("attention"));
    assert!(!json.contains("\"target\""));
    assert!(
        !serde_json::to_string(&desktop)
            .unwrap()
            .contains("window_sample")
    );
    assert!(
        !serde_json::to_string(&desktop)
            .unwrap()
            .contains("cursor_sample_millis")
    );
    let restored = World::from_save(serde_json::from_str(&json).unwrap());
    assert!(
        restored
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_none())
    );
    assert!(restored.attention.plans.is_empty());
}

#[test]
fn sleeping_distant_occluded_and_uninterested_companions_are_not_recruited() {
    for reason in 0..4 {
        let (mut world, mut desktop, now) = scene();
        let observer = world.save.creatures[2].id;
        match reason {
            0 => world.save.creatures[2].state.action = ActionKind::Sleep,
            1 => world.save.creatures[2].state.position.x = 1_350.0,
            2 => {
                desktop.windows[0].z_order = 1;
                desktop.windows.push(DesktopWindow {
                    key: 702,
                    bounds: DesktopRect {
                        x: 560.0,
                        y: 780.0,
                        width: 140.0,
                        height: 100.0,
                    },
                    z_order: 0,
                    visible: true,
                    minimized: false,
                    application: None,
                    application_name: None,
                });
            }
            _ => {
                world.save.creatures[2].personality.curiosity = 0.0;
                world.save.creatures[2].personality.sociability = 0.0;
                for bond in &mut world.save.relationships {
                    if bond.a == observer || bond.b == observer {
                        bond.affinity = 0;
                    }
                }
            }
        }
        start(&mut world, &mut desktop, now);
        assert!(
            !world.attention.owns(observer),
            "ineligible observer, reason {reason}"
        );
        assert!(
            world
                .attention
                .plans
                .values()
                .any(|plan| matches!(plan.role, Role::Actor { .. }))
        );
    }
}
pub(super) fn open_scene() -> (World, DesktopSnapshot, OffsetDateTime) {
    let (mut world, mut desktop, now) = scene();
    desktop.windows.clear();
    world.clear_attention();
    world.geometry_observer = crate::attention::GeometryObserver::default();
    world.geometry_observer.update(&desktop, 0.05, true);
    for (index, creature) in world.save.creatures.iter_mut().enumerate() {
        creature.state.surface = SurfaceAttachment {
            kind: SurfaceKind::ScreenFloor,
            monitor_id: 1,
            window_key: None,
            relative_x: 0.5,
        };
        creature.state.position = Point {
            x: 480.0 - index as f32 * 65.0,
            y: 846.0,
        };
        creature.personality.activity = 0.7;
        creature.state.action = ActionKind::Idle;
        creature.state.action_duration = 100.0;
        if index > 0 {
            creature.personality.curiosity = 0.7;
        }
    }
    (world, desktop, now)
}

fn new_window(world: &mut World, desktop: &mut DesktopSnapshot, now: OffsetDateTime) {
    desktop.windows.push(DesktopWindow {
        key: 702,
        bounds: DesktopRect {
            x: 580.0,
            y: 650.0,
            width: 240.0,
            height: 150.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 250;
    world.tick(now + Duration::milliseconds(250), 0.05, desktop);
}

#[test]
fn new_edge_gets_a_short_approach_and_companions_reserve_distinct_viewing_spots() {
    let (mut world, mut desktop, now) = open_scene();
    new_window(&mut world, &mut desktop, now);
    let actor = world.save.creatures[0].id;
    let observer = world.save.creatures[1].id;
    let destination = world.attention.plans[&actor].walk.unwrap().destination;
    let viewing_spot = world.attention.plans[&observer].walk.unwrap().destination;
    assert!(destination.x > 480.0 && destination.x < 580.0);
    assert!(viewing_spot.x > 415.0 && viewing_spot.x < destination.x - 24.0);
    assert_eq!(world.save.creatures[0].state.position.x, 480.0); // Notice first.
    for step in 1..=120 {
        world.tick(
            now + Duration::milliseconds(250 + step * 50),
            0.05,
            &desktop,
        );
        let a = &world.save.creatures[0];
        let b = &world.save.creatures[1];
        assert!(a.state.position.distance(b.state.position) >= 24.0);
        assert_eq!(a.state.surface.window_key, None);
        if step == 20 {
            assert!(a.state.position.x > 480.0);
            assert_eq!(a.state.action, ActionKind::Traverse);
            assert!(a.state.velocity.x > 0.0);
            assert!(b.state.position.x > 415.0);
            assert!(
                matches!(world.attention.plans[&observer].role, Role::Observer { actor: id } if id == actor)
            );
        }
        if step == 65 {
            assert!((a.state.position.x - destination.x).abs() < 1.0);
            assert_eq!(a.state.action, ActionKind::InspectScreen);
            assert!(a.state.attention.is_some());
        }
    }
    assert!(world.attention.plans.is_empty());
    assert!(
        world
            .save
            .creatures
            .iter()
            .all(|c| c.state.attention.is_none())
    );
}

#[test]
fn changed_geometry_or_a_blocking_companion_ends_an_approach_at_a_safe_point() {
    for blocked_by_creature in [false, true] {
        let (mut world, mut desktop, now) = open_scene();
        new_window(&mut world, &mut desktop, now);
        for step in 1..=20 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
        }
        let id = world.save.creatures[0].id;
        let before = world.save.creatures[0].state.position;
        if blocked_by_creature {
            let other = world.save.creatures[1].id;
            world.cancel_creature_attention(other);
            world.save.creatures[1].state.position.x = before.x + 24.0;
            world.save.creatures[1].state.action = ActionKind::Sleep;
            world.save.creatures[1].state.action_duration = 100.0;
        } else {
            desktop.windows[0].bounds.x += 60.0;
            desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_500;
        }
        world.tick(now + Duration::milliseconds(1_500), 0.05, &desktop);
        assert!(world.attention.plans[&id].walk.is_none());
        assert_eq!(world.save.creatures[0].state.position, before);
        assert_eq!(world.save.creatures[0].state.velocity, Point::default());
        assert_eq!(
            world.save.creatures[0].state.action,
            ActionKind::InspectScreen
        );
    }
}

#[test]
fn approaches_respect_excluded_strips_and_reduced_motion() {
    for reduced_motion in [false, true] {
        let (mut world, mut desktop, now) = open_scene();
        world.save.settings.reduce_motion = reduced_motion;
        if !reduced_motion {
            world.save.settings.habitat.zones.push(HabitatZone {
                id: 991,
                display: desktop.monitors[0].display_key,
                kind: HabitatZoneKind::Excluded,
                enabled: true,
                normalized_bounds: DesktopRect {
                    x: 510.0 / 1440.0,
                    y: 0.0,
                    width: 24.0 / 1440.0,
                    height: 1.0,
                },
            });
        }
        new_window(&mut world, &mut desktop, now);
        let actor = world.save.creatures[0].id;
        assert!(world.attention.plans[&actor].walk.is_none());
        for step in 1..=20 {
            world.tick(
                now + Duration::milliseconds(250 + step * 50),
                0.05,
                &desktop,
            );
        }
        assert_eq!(world.save.creatures[0].state.position.x, 480.0);
        assert!(world.save.creatures[0].state.attention.is_some());
    }
}

#[test]
fn growth_makes_a_timid_neighbor_retreat_while_a_bold_one_inspects() {
    for bold in [false, true] {
        let (mut world, mut desktop, now) = open_scene();
        new_window(&mut world, &mut desktop, now);
        world.clear_attention();
        world.save.creatures.truncate(1);
        let creature = &mut world.save.creatures[0];
        creature.personality.boldness = f32::from(bold);
        creature.personality.window_tolerance = f32::from(bold);
        creature.state.position.x = 500.0;
        desktop.windows[0].bounds.width += 180.0;
        desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
        world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
        let id = world.save.creatures[0].id;
        assert_eq!(
            world.attention.plans[&id].emotion,
            if bold {
                AttentionEmotion::Curious
            } else {
                AttentionEmotion::Startled
            }
        );
        for step in 1..=25 {
            world.tick(
                now + Duration::milliseconds(500 + step * 50),
                0.05,
                &desktop,
            );
        }
        let creature = &world.save.creatures[0];
        if bold {
            assert_eq!(creature.state.position.x, 500.0);
        } else {
            assert!(creature.state.position.x < 480.0);
        }
        assert_eq!(creature.state.surface.kind, SurfaceKind::ScreenFloor);
    }
}

#[test]
fn recently_lost_support_prompts_one_search_even_during_the_ride_cooldown() {
    let (mut world, mut desktop, now) = scene();
    start(&mut world, &mut desktop, now);
    let actor = world.save.creatures[0].id;
    let old_origin = world.attention.plans[&actor].origin;
    desktop.windows.clear();
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 500;
    world.tick(now + Duration::milliseconds(500), 0.05, &desktop);
    assert!(!world.attention.owns(actor));
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 750;
    world.tick(now + Duration::milliseconds(750), 0.05, &desktop);
    assert!(world.attention.cooldowns.contains_key(&actor));
    assert!(matches!(
        world.attention.plans[&actor].role,
        Role::Actor { vanished: true, .. }
    ));
    assert_ne!(world.attention.plans[&actor].origin, old_origin);
    let mut glances = Vec::new();
    for step in 1..=80 {
        world.tick(
            now + Duration::milliseconds(750 + step * 50),
            0.05,
            &desktop,
        );
        if [12, 25, 38].contains(&step) {
            glances.push(world.save.creatures[0].state.attention.unwrap().target.x);
        }
    }
    assert_ne!(glances[0], glances[1]);
    assert_eq!(glances[0], glances[2]);
    assert!(world.attention.plans.is_empty());
    assert!(world.save.creatures[0].state.attention.is_none());
}
/// S10: the same scene with two, three, and four creatures, through both outcomes.
#[test]
fn spectator_sequences_stay_readable_at_every_colony_size_and_outcome() {
    for colony in 2..=4 {
        for bold in [false, true] {
            let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(true, bold);
            world.save.creatures.truncate(colony);
            let actor = world.save.creatures[0].id;
            let mut seen: BTreeMap<CreatureId, Vec<AttentionEmotion>> = BTreeMap::new();
            let mut celebrated = false;
            for step in 3..200 {
                desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
                world.tick(
                    now + Duration::milliseconds(step as i64 * 50),
                    0.05,
                    &desktop,
                );
                for creature in world.save.creatures.iter().skip(1) {
                    let watching = world
                        .attention
                        .plans
                        .get(&creature.id)
                        .is_some_and(|p| matches!(p.role, Role::Observer { .. }));
                    if let Some(pose) = creature.state.attention.filter(|_| watching) {
                        let log = seen.entry(creature.id).or_default();
                        if log.last() != Some(&pose.emotion) {
                            log.push(pose.emotion);
                        }
                        celebrated |= creature.state.action == ActionKind::Greet;
                    }
                }
            }
            let label = format!("colony {colony}, bold {bold}");
            assert!(!seen.is_empty(), "somebody watches: {label}");
            for log in seen.values() {
                assert_eq!(log[0], AttentionEmotion::Curious, "notice first: {label}");
                assert!(log.len() >= 2, "a response follows: {label}");
                assert!(
                    log.iter().any(|e| matches!(
                        e,
                        AttentionEmotion::Concerned
                            | AttentionEmotion::Averting
                            | AttentionEmotion::Startled
                            | AttentionEmotion::Enjoying
                    )),
                    "a felt response: {label}"
                );
                // Everyone ends calm; only a playful watcher turns that into delight.
                assert!(
                    matches!(
                        log.last(),
                        Some(AttentionEmotion::Relieved | AttentionEmotion::Enjoying)
                    ),
                    "settles at the end: {label}"
                );
            }
            let delighted = seen.iter().any(|(id, log)| {
                log.last() == Some(&AttentionEmotion::Enjoying)
                    && world
                        .save
                        .creatures
                        .iter()
                        .any(|c| c.id == *id && c.personality.playfulness > 0.65)
            });
            assert_eq!(delighted, bold, "delight follows a real success: {label}");
            // Only a real success is celebrated, and nobody keeps watching forever.
            assert_eq!(celebrated, bold, "celebration matches outcome: {label}");
            assert!(world.attention.plans.is_empty(), "{label}");
            assert!(
                world
                    .save
                    .creatures
                    .iter()
                    .all(|c| c.state.attention.is_none()),
                "{label}"
            );
            assert_eq!(
                world.save.creatures[0].state.surface.window_key,
                Some(if bold { 702 } else { 701 }),
                "{label}"
            );
            let _ = actor;
        }
    }
}

#[test]
fn an_actor_that_becomes_hidden_loses_its_audience() {
    let (mut world, mut desktop, now) = super::ledges::tests::edge_scene(true, true);
    let actor = world.save.creatures[0].id;
    let mut watched = false;
    for step in 3..40 {
        desktop.window_sample.as_mut().unwrap().monotonic_millis = step * 50;
        world.tick(
            now + Duration::milliseconds(step as i64 * 50),
            0.05,
            &desktop,
        );
        watched |= world
            .attention
            .plans
            .values()
            .any(|p| matches!(p.role, Role::Observer { actor: watched } if watched == actor));
        if watched {
            break;
        }
    }
    assert!(watched, "the attempt gathers an audience first");
    // A window slides in front of the actor: its watchers cannot see it any more.
    desktop.windows[0].z_order = 3;
    desktop.windows[1].z_order = 4;
    desktop.windows.push(DesktopWindow {
        key: 703,
        bounds: DesktopRect {
            x: 700.0,
            y: 500.0,
            width: 240.0,
            height: 220.0,
        },
        z_order: 0,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 2_000;
    world.tick(now + Duration::milliseconds(2_000), 0.05, &desktop);
    assert!(
        !world
            .attention
            .plans
            .values()
            .any(|p| matches!(p.role, Role::Observer { .. })),
        "a hidden actor is not watched"
    );
    assert!(world.window_journeys.is_empty());
}

#[test]
fn attention_walks_keep_clear_of_an_existing_landing_reservation() {
    let (mut world, mut desktop, now) = open_scene();
    let jumper = world.save.creatures[3].id;
    world.window_journeys.insert(
        jumper,
        WindowJourney::Hop(HopJourney {
            start: Point { x: 100.0, y: 600.0 },
            target: Point { x: 556.0, y: 846.0 },
            surface: world.save.creatures[0].state.surface.clone(),
            elapsed: 0.0,
            duration: 3.0,
        }),
    );
    new_window(&mut world, &mut desktop, now);
    let id = world.save.creatures[0].id;
    assert!(world.attention.plans[&id].walk.is_none());
    assert_eq!(world.save.creatures[0].state.position.x, 480.0);
}

#[test]
fn an_approach_on_a_ledge_cancels_immediately_when_that_support_vanishes() {
    let (mut world, mut desktop, now) = open_scene();
    desktop.windows.push(DesktopWindow {
        key: 701,
        bounds: DesktopRect {
            x: 200.0,
            y: 600.0,
            width: 600.0,
            height: 200.0,
        },
        z_order: 1,
        visible: true,
        minimized: false,
        application: None,
        application_name: None,
    });
    let creature = &mut world.save.creatures[0];
    creature.state.surface = SurfaceAttachment {
        monitor_id: 1,
        window_key: Some(701),
        kind: SurfaceKind::WindowLedge,
        relative_x: (480.0 - 200.0) / 600.0,
    };
    creature.state.position.y = 600.0;
    world.geometry_observer = crate::attention::GeometryObserver::default();
    world.geometry_observer.update(&desktop, 0.05, true);
    new_window(&mut world, &mut desktop, now);
    let id = world.save.creatures[0].id;
    assert!(world.attention.plans[&id].walk.is_some());
    for step in 1..=20 {
        world.tick(
            now + Duration::milliseconds(250 + step * 50),
            0.05,
            &desktop,
        );
    }
    assert!(world.save.creatures[0].state.position.x > 480.0);
    desktop.windows.retain(|w| w.key != 701);
    desktop.window_sample.as_mut().unwrap().monotonic_millis = 1_500;
    world.tick(now + Duration::milliseconds(1_500), 0.05, &desktop);
    assert!(!world.attention.owns(id));
    assert_eq!(
        world.save.creatures[0].state.surface.kind,
        SurfaceKind::ScreenFloor
    );
    assert!(world.save.creatures[0].state.attention.is_none());
}
