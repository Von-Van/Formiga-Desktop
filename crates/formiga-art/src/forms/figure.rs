//! Where each piece of a sculpted body goes in one pose: the body, the head, the neck, the limbs
//! and the root of the tail. A plan sets the body out its own way; the proportions stretch it
//! within the plan's authored range.

use super::motion::Pose;
use formiga_core::forms::{MIDDLE, Plan, STEPS, Sculpt};

/// The row a standing form's feet rest on.
pub(crate) const FLOOR: f32 = 41.0;

/// How far a floater hovers off the ground at rest.
pub(crate) const HOVER: f32 = 3.0;

/// A slider's step as -1 (least) to 1 (most), 0 being the plan's own.
fn t(value: u8) -> f32 {
    (f32::from(value.min(STEPS)) - f32::from(MIDDLE)) / f32::from(MIDDLE)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limb {
    pub(crate) top: (f32, f32),
    pub(crate) foot: (f32, f32),
    pub(crate) radius: f32,
    /// On the near side, drawn over the body; or the far side, behind it and in shade.
    pub(crate) near: bool,
    pub(crate) front: bool,
}

/// A neck: from the body, to the head, and its radius.
pub(crate) type Neck = ((f32, f32), (f32, f32), f32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Body {
    pub(crate) cx: f32,
    pub(crate) cy: f32,
    pub(crate) rx: f32,
    pub(crate) ry: f32,
    pub(crate) angle: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Head {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) r: f32,
}

/// A body set out in one pose.
#[derive(Clone, Debug)]
pub(crate) struct Figure {
    pub(crate) plan: Plan,
    pub(crate) scale: f32,
    pub(crate) body: Body,
    pub(crate) head: Head,
    /// From where it leaves the body to the middle of the head, and how thick.
    pub(crate) neck: Option<Neck>,
    pub(crate) legs: Vec<Limb>,
    pub(crate) arms: Vec<Limb>,
    /// Where the tail leaves the body, and the way it points (a unit vector, backwards).
    pub(crate) tail_root: (f32, f32),
    pub(crate) tail_length: f32,
    /// The middle of the face.
    pub(crate) face: (f32, f32),
    /// The point someone picks it up by.
    pub(crate) scruff: (f32, f32),
}

impl Figure {
    pub(crate) fn lay_out(sculpt: &Sculpt, scale: f32, pose: Pose) -> Self {
        let shape = sculpt.shape;
        let (tl, td, tn, tg, tt) = (
            t(shape.length),
            t(shape.depth),
            t(shape.neck),
            t(shape.girth),
            t(shape.tail),
        );
        let (th, tleg, tarm) = (t(shape.head), t(shape.legs), t(shape.arms));
        let s = scale;
        let floor = FLOOR;
        // Hanging from the scruff, everything is lifted clear of the ground, and the legs let
        // go of it.
        let held_lift = if pose.held { 5.0 } else { 0.0 };
        let lift = pose.lift + held_lift;
        let mut legs = Vec::new();
        let mut arms = Vec::new();
        let mut neck = None;
        let cx = 22.0;
        let crouch = pose.crouch;
        let (body, head, tail_length);
        match sculpt.plan {
            Plan::CompactQuadruped | Plan::LargeQuadruped | Plan::TallQuadruped => {
                let (rx, ry, leg, lr, hr, neck_length, tail) = match sculpt.plan {
                    Plan::CompactQuadruped => (
                        10.0 + 2.5 * tl,
                        7.0 + 1.5 * td,
                        6.0 + 2.5 * tleg,
                        2.3 + 0.8 * tg,
                        9.5 + 1.5 * th,
                        2.0 + 2.0 * tn,
                        6.0 + 3.0 * tt,
                    ),
                    Plan::LargeQuadruped => (
                        13.0 + 2.5 * tl,
                        9.0 + 1.5 * td,
                        5.0 + 2.0 * tleg,
                        3.6 + 0.9 * tg,
                        10.0 + 1.5 * th,
                        0.8 + 1.0 * tn,
                        5.0 + 2.0 * tt,
                    ),
                    _ => (
                        8.0 + 2.0 * tl,
                        5.0 + 1.2 * td,
                        10.0 + 3.0 * tleg,
                        1.8 + 0.5 * tg,
                        7.5 + 1.0 * th,
                        11.0 + 4.0 * tn,
                        6.0 + 2.0 * tt,
                    ),
                };
                let (rx, ry, leg, lr, hr) = (rx * s, ry * s, leg * s, lr * s, (hr * s).max(7.5));
                let (neck_length, tail) = (neck_length * s, tail * s);
                let standing = leg * (1.0 - 0.75 * crouch);
                let cy = floor - lift - standing - ry * 0.45 + pose.bob;
                let body_ = Body {
                    cx,
                    cy,
                    rx,
                    ry,
                    angle: -pose.tip,
                };
                for (index, (front, near)) in
                    [(true, true), (false, true), (true, false), (false, false)]
                        .into_iter()
                        .enumerate()
                {
                    let x = cx
                        + if front { rx * 0.58 } else { -rx * 0.55 }
                        + if near { 0.0 } else { 2.0 };
                    let top = (x, cy + ry * 0.3 - if near { 0.0 } else { 1.0 });
                    let foot = if pose.held {
                        (x + pose.step[index], top.1 + leg * 0.9)
                    } else {
                        (
                            x + pose.step[index] * s,
                            floor - lift - pose.raise[index] - if near { 0.0 } else { 1.0 },
                        )
                    };
                    legs.push(Limb {
                        top,
                        foot,
                        radius: lr,
                        near,
                        front,
                    });
                }
                let base = (cx + rx * 0.62, cy - ry * 0.35);
                let center = if sculpt.plan == Plan::TallQuadruped {
                    let (sin, cos) = (1.3_f32 - pose.nod * 0.15).sin_cos();
                    let reach = neck_length + hr * 0.6;
                    (base.0 + cos * reach + pose.lean, base.1 - sin * reach)
                } else if sculpt.plan == Plan::LargeQuadruped {
                    (
                        base.0 + hr * 0.45 + neck_length * 0.5 + pose.lean,
                        base.1 - hr * 0.25 - neck_length * 0.8 + pose.nod,
                    )
                } else {
                    (
                        base.0 + neck_length * 0.45 + hr * 0.3 + pose.lean,
                        base.1 - neck_length * 0.9 - hr * 0.5 + pose.nod,
                    )
                };
                let center = (center.0, center.1 + crouch * 2.0);
                if neck_length > 1.5 {
                    neck = Some((base, center, (lr * 1.25).max(hr * 0.35)));
                }
                body = body_;
                head = Head {
                    x: center.0,
                    y: center.1,
                    r: hr,
                };
                tail_length = tail;
            }
            Plan::Upright => {
                let rx = (7.5 + 1.5 * tl) * s;
                let ry = (9.0 + 2.0 * td) * s;
                let leg = (4.0 + 2.0 * tleg) * s;
                let lr = (2.5 + 0.8 * tg) * s;
                let hr = ((10.0 + 1.5 * th) * s).max(7.5);
                let arm = (9.0 + 4.0 * tarm) * s;
                let neck_length = ((0.5 + 1.5 * tn) * s).max(0.0);
                let sit = crouch * leg;
                let cy = floor - lift - leg - ry * 0.8 + pose.bob + sit;
                let cx = cx + 2.0;
                body = Body {
                    cx,
                    cy,
                    rx,
                    ry,
                    angle: pose.tip,
                };
                for (index, near) in [(0, true), (1, false)] {
                    let x = cx + if near { rx * 0.4 } else { -rx * 0.4 };
                    let top = (x, cy + ry * 0.55);
                    let foot = if pose.held {
                        (x + pose.step[index], top.1 + leg + 2.0)
                    } else if crouch > 0.5 {
                        // Sat down, legs out in front.
                        (x + leg * 0.9, floor - lift - lr * 0.6)
                    } else {
                        (x + pose.step[index] * s, floor - lift - pose.raise[index])
                    };
                    legs.push(Limb {
                        top,
                        foot,
                        radius: lr,
                        near,
                        front: true,
                    });
                }
                for near in [true, false] {
                    let side = if near { 1.0 } else { -1.0 };
                    let shoulder = (cx + side * rx * 0.7, cy - ry * 0.45);
                    // From hanging at the sides, swung up and out as far as the pose asks.
                    let angle = pose.spread * 2.2;
                    let (sin, cos) = angle.sin_cos();
                    let mut hand = (
                        shoulder.0 + side * (arm * 0.25 + sin * arm * 0.6),
                        shoulder.1 + cos * arm,
                    );
                    hand.1 = hand.1.min(floor - lift - 1.0);
                    arms.push(Limb {
                        top: shoulder,
                        foot: hand,
                        radius: (lr * 0.95).max(1.8),
                        near,
                        front: true,
                    });
                }
                let center = (
                    cx + 1.5 + pose.lean,
                    cy - ry - hr * 0.55 + 2.5 - neck_length + pose.nod,
                );
                head = Head {
                    x: center.0,
                    y: center.1,
                    r: hr,
                };
                tail_length = (5.0 + 2.5 * tt) * s;
            }
            Plan::Floater => {
                let rx = (15.0 + 3.0 * tl) * s;
                let ry = (7.5 + 2.0 * td) * s;
                let hr = ((8.0 + 1.5 * th) * s).max(7.5);
                let hover = HOVER * (1.0 - crouch);
                let cy = floor - lift - hover - ry - 0.5 + pose.bob * 0.0;
                body = Body {
                    cx: cx - 1.0,
                    cy,
                    rx,
                    ry,
                    angle: -pose.tip,
                };
                head = Head {
                    x: cx - 1.0 + rx * 0.5 + pose.lean,
                    y: cy - ry * 0.18 + pose.nod * 0.5,
                    r: hr,
                };
                tail_length = (9.0 + 4.0 * tt) * s;
            }
            Plan::Crawler => {
                let rx = (12.0 + 2.0 * tl) * s;
                let ry = (8.0 + 2.0 * td) * s;
                let leg = (3.0 + 1.5 * tleg) * s;
                let lr = (2.2 + 0.6 * tg) * s;
                let hr = ((8.0 + 1.0 * th) * s).max(7.5);
                let neck_length = (3.0 + 2.0 * tn) * s * (1.0 - 0.6 * crouch);
                let cy = floor - lift - leg * (1.0 - 0.6 * crouch) - ry * 0.35 + pose.bob;
                body = Body {
                    cx,
                    cy,
                    rx,
                    ry,
                    angle: -pose.tip,
                };
                for (index, (front, near)) in
                    [(true, true), (false, true), (true, false), (false, false)]
                        .into_iter()
                        .enumerate()
                {
                    let x = cx
                        + if front { rx * 0.55 } else { -rx * 0.6 }
                        + if near { 0.0 } else { 2.0 };
                    let top = (x, cy + ry * 0.2);
                    let foot = if pose.held {
                        (x + pose.step[index], top.1 + leg + 2.0)
                    } else {
                        (
                            x + pose.step[index] * s + if front { 1.5 } else { -1.0 },
                            floor - lift - pose.raise[index] - if near { 0.0 } else { 1.0 },
                        )
                    };
                    legs.push(Limb {
                        top,
                        foot,
                        radius: lr,
                        near,
                        front,
                    });
                }
                let base = (cx + rx * 0.8, cy + ry * 0.05);
                let center = (
                    base.0 + neck_length + hr * 0.45 + pose.lean,
                    base.1 - hr * 0.35 + pose.nod,
                );
                neck = Some((base, center, (hr * 0.45).max(2.5)));
                head = Head {
                    x: center.0,
                    y: center.1,
                    r: hr,
                };
                tail_length = (4.0 + 2.0 * tt) * s;
            }
            Plan::Percher => {
                let rx = (7.5 + 1.5 * tl) * s;
                let ry = (9.5 + 2.0 * td) * s;
                let leg = (4.0 + 2.5 * tleg) * s;
                let hr = ((9.0 + 1.5 * th) * s).max(7.5);
                let neck_length = (0.5 + 1.0 * tn) * s;
                let standing = leg * (1.0 - 0.8 * crouch);
                let cy = floor - lift - standing - ry * 0.85 + pose.bob;
                body = Body {
                    cx,
                    cy,
                    rx,
                    ry,
                    angle: 0.3 + pose.tip,
                };
                for (index, near) in [(0, true), (1, false)] {
                    let x = cx + if near { 1.5 } else { -1.5 };
                    let top = (x, cy + ry * 0.7);
                    let foot = if pose.held {
                        (x + pose.step[index], top.1 + leg + 1.0)
                    } else {
                        (x + pose.step[index] * s, floor - lift)
                    };
                    legs.push(Limb {
                        top,
                        foot,
                        radius: 1.0,
                        near,
                        front: true,
                    });
                }
                head = Head {
                    x: cx + rx * 0.4 + pose.lean,
                    y: cy - ry - hr * 0.3 + 2.0 - neck_length + pose.nod,
                    r: hr,
                };
                tail_length = (6.0 + 3.0 * tt) * s;
            }
        }
        let tail_root = match sculpt.plan {
            Plan::Upright => (body.cx - body.rx * 0.8, body.cy + body.ry * 0.6),
            Plan::Percher => (body.cx - body.rx * 0.5, body.cy + body.ry * 0.55),
            Plan::Floater => (body.cx - body.rx * 0.8, body.cy + body.ry * 0.05),
            Plan::Crawler => (body.cx - body.rx * 0.9, body.cy + body.ry * 0.15),
            _ => (body.cx - body.rx * 0.85, body.cy - body.ry * 0.25),
        };
        let face = match sculpt.plan {
            Plan::Floater => (head.x + head.r * 0.25, head.y + head.r * 0.05),
            _ => (head.x + head.r * 0.06, head.y + head.r * 0.1),
        };
        let scruff = match sculpt.plan {
            Plan::Floater | Plan::Crawler => (body.cx, body.cy - body.ry),
            _ => (
                (body.cx + head.x) / 2.0,
                (body.cy - body.ry).min(head.y - head.r * 0.5),
            ),
        };
        Self {
            plan: sculpt.plan,
            scale,
            body,
            head,
            neck,
            legs,
            arms,
            tail_root,
            tail_length,
            face,
            scruff,
        }
    }

    /// Everything shifted across the frame.
    pub(crate) fn shifted(self, dx: f32) -> Self {
        self.moved(dx, 0.0)
    }

    /// The same figure, everything in it moved by `dx` and `dy`.
    pub(crate) fn moved(mut self, dx: f32, dy: f32) -> Self {
        let mv = |p: &mut (f32, f32)| {
            p.0 += dx;
            p.1 += dy;
        };
        self.body.cx += dx;
        self.body.cy += dy;
        self.head.x += dx;
        self.head.y += dy;
        if let Some((a, b, _)) = &mut self.neck {
            mv(a);
            mv(b);
        }
        for limb in self.legs.iter_mut().chain(self.arms.iter_mut()) {
            mv(&mut limb.top);
            mv(&mut limb.foot);
        }
        mv(&mut self.tail_root);
        mv(&mut self.face);
        mv(&mut self.scruff);
        self
    }
}
