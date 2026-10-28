//! Keeping each creature's [`Presence`] up to date: whether its own feet are carrying it, since
//! when, how far, and a clock for its eyes.
//!
//! Every action moves a creature its own way — a walk by its velocity, a journey home along a
//! path, a shuffle out of a friend's way, a window carrying whoever stands on it — so the one
//! honest measure of whether it is walking is where it has got to. This pass reads that at the
//! end of each tick and writes nothing anything else in the simulation reads, so it cannot change
//! what a colony does: only how the art draws it doing it.

use super::*;

/// Slower than this along the ground, in points a second, a creature is standing, however it is
/// being nudged. The slowest walk anything does is a tow at sixteen.
const STEPPING_PACE: f32 = 6.0;
/// How long a change between stepping and standing has to hold before it shows, in seconds:
/// two ticks of a moving colony, so a single tick's nudge is never drawn as a step.
const SETTLE_SECS: f32 = 0.1;
/// How long a creature keeps stepping or standing at the least, once it has started, in seconds,
/// so one hovering at the edge of where it means to stop is not drawn flickering between the two.
const HOLD_SECS: f32 = 0.25;
/// Further than this in one tick, in points, is a jump — a hop across a gap, a new display, a
/// colony arriving — and is not counted as walking.
const JUMP_POINTS: f32 = 40.0;
/// How long one whole walking stride is, against the body's own size: a little under half of
/// it. Six frames make a stride, so this sets how fast the feet go for how fast the ground goes
/// by. At the default size and an average companion's pace it is the ten frames a second the walk
/// has always played at, so the walk itself looks as it did; what changes is that a slow
/// stroller steps slowly, a busy one briskly, and a mini patters.
const WALK_STRIDE: f32 = 0.4;
/// A run's stride, against the body: twice a walk's and a little more, its legs stretched out.
const RUN_STRIDE: f32 = 0.83;
/// However fast or slow the ground goes by, the feet keep within these, in frames a second: any
/// slower and a step reads as the picture sticking, any faster as a blur.
const WALK_FRAMES_PER_SEC: std::ops::RangeInclusive<f32> = 7.0..=15.0;
const RUN_FRAMES_PER_SEC: std::ops::RangeInclusive<f32> = 9.0..=18.0;
/// The clock goes round every two hours, which keeps it precise; the eyes never notice.
const CLOCK_ROUND_SECS: f32 = 7_200.0;
/// The stride count goes round too, at a whole number of six-frame strides.
const STRIDE_ROUND: f32 = 6.0 * 10_000.0;

impl World {
    /// Measure where everyone has got to since the last tick. Called once, at the end of every
    /// tick, paused or not.
    pub(super) fn carry_presence(&mut self, dt: f32, desktop: &DesktopSnapshot) {
        let display_scale = f32::from(self.save.settings.display_scale.max(1));
        for creature in &mut self.save.creatures {
            let pixels_per_point = desktop
                .monitors
                .iter()
                .find(|monitor| monitor.id == creature.state.surface.monitor_id)
                .map_or(1.0, |monitor| monitor.scale_factor.max(0.1))
                / display_scale;
            carry(creature, dt, desktop, pixels_per_point);
        }
    }
}

/// Where a creature is standing now, as [`Presence`] measures it.
fn mark(creature: &Creature, desktop: &DesktopSnapshot) -> PresenceMark {
    let surface = &creature.state.surface;
    PresenceMark {
        x: creature.state.position.x,
        window: surface.window_key.and_then(|key| {
            desktop
                .windows
                .iter()
                .find(|window| window.key == key)
                .map(|window| (key, surface.relative_x * window.bounds.width))
        }),
    }
}

/// Whether what a creature is doing moves it some other way than on its own feet: in a hand, in
/// the air, up a window, hanging from one or carried along on it, or out of sight.
fn carried(creature: &Creature) -> bool {
    creature.state.indoors
        || creature.state.arrival_delay_secs > 0.0
        || creature
            .state
            .attention
            .is_some_and(|pose| pose.hanging > 0.0)
        || matches!(
            creature.state.action,
            ActionKind::Dragged
                | ActionKind::Tossed
                | ActionKind::Landing
                | ActionKind::ClimbWindow
                | ActionKind::Dangle
                | ActionKind::RideWindow
        )
        // Asleep, it is only ever towed or wriggling over; on its way to bed it walks.
        || (creature.state.action == ActionKind::Sleep && !creature.state.walking_to_sleep())
}

pub(super) fn carry(
    creature: &mut Creature,
    dt: f32,
    desktop: &DesktopSnapshot,
    pixels_per_point: f32,
) {
    let now = mark(creature, desktop);
    let carried = carried(creature);
    let logical_size = f32::from(creature.appearance.logical_size.max(8));
    let running = creature.state.action == ActionKind::Sprint;
    // Setting out on a walk shows from its first step: the walk says it is going somewhere, and
    // its feet agree. Anything else that moves it waits a moment to be sure.
    let setting_out = creature.state.walking_to_sleep()
        || matches!(
            creature.state.action,
            ActionKind::Traverse
                | ActionKind::Follow
                | ActionKind::Sprint
                | ActionKind::SqueezeWindow
        );
    let presence = &mut creature.state.presence;
    let dt = dt.max(0.0);
    presence.clock = (presence.clock + dt) % CLOCK_ROUND_SECS;
    presence.since += dt;
    let walked = match presence.last {
        // Along the same window, how far along it; anywhere else, how far across.
        Some(PresenceMark {
            window: Some((was_on, was_at)),
            ..
        }) if now.window.is_some_and(|(key, _)| key == was_on) => {
            now.window.map_or(0.0, |(_, at)| (at - was_at).abs())
        }
        Some(was) => (now.x - was.x).abs(),
        None => 0.0,
    };
    let first = presence.last.is_none();
    presence.last = Some(now);
    let moving =
        !first && !carried && dt > 0.0 && walked <= JUMP_POINTS && walked >= STEPPING_PACE * dt;
    if moving {
        let (stride, rates) = if running {
            (RUN_STRIDE, RUN_FRAMES_PER_SEC)
        } else {
            (WALK_STRIDE, WALK_FRAMES_PER_SEC)
        };
        let frame_length = logical_size * stride / 6.0;
        let frames =
            (walked * pixels_per_point / frame_length).clamp(rates.start() * dt, rates.end() * dt);
        presence.stride = (presence.stride + frames) % STRIDE_ROUND;
    }
    if carried {
        // Whatever carried it, it lands standing.
        if presence.stepping {
            presence.stepping = false;
            presence.since = 0.0;
        }
        presence.pending = 0.0;
        return;
    }
    if moving == presence.stepping {
        presence.pending = 0.0;
        return;
    }
    presence.pending += dt;
    let sure = (moving && setting_out) || presence.pending >= SETTLE_SECS;
    if sure && presence.since >= HOLD_SECS {
        presence.stepping = moving;
        presence.since = 0.0;
        presence.pending = 0.0;
        if moving {
            // Setting off starts on the stride's first footfall.
            presence.stride = 0.0;
        }
    }
}
