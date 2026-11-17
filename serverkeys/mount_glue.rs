// Server keys (module `crate::game::serverkeys_mount`): the horse this player drives moves on this client (the server only takes where it
// went), and Horizon moves a mob by its type's gravity and step, not by attribute modifiers. So
// progression's horse perks are carried out here: "Впевнений спуск" makes the driven horse fall
// many times faster (it keeps to the ground down slopes and off ledges) with a jump that still
// clears the same height, "Прохідливість" lets it step up higher. Nothing changes without them.
use super::Game;
use horizon_client::mobs::equine;

pub(super) fn drive(game: &mut Game<'_>, i: usize) {
    let gravity_x = f64::from(horizon_client::serverkeys_flags::horse_gravity());
    let step_add = f64::from(horizon_client::serverkeys_flags::horse_step());
    let Some(m) = game.mobs.get_mut(i) else {
        return;
    };
    if !equine::is_equine(m.species()) {
        return;
    }
    let mut seen = serverkeys_mount_seen().lock().unwrap_or_else(|p| p.into_inner());
    let base = *seen.entry(m.id).or_insert(SkBase {
        gravity: m.kind.gravity,
        step: m.kind.step_height,
        jump: m.extra.vector[0],
        jump_set: m.extra.vector[0],
    });
    // A jump strength the server sent since (a new horse, /attribute) is the new base.
    let jump_base = if (m.extra.vector[0] - base.jump_set).abs() > 1e-9 { m.extra.vector[0] } else { base.jump };
    // In water and lava the horse swims as vanilla's does.
    let x = if m.fluids.in_water || m.fluids.in_lava { 1.0 } else { gravity_x.max(1.0) };
    let gravity = base.gravity * x;
    let step = base.step + step_add;
    if (m.kind.gravity - gravity).abs() > 1e-12 || (m.kind.step_height - step).abs() > 1e-12 {
        let kind = std::sync::Arc::make_mut(&mut m.kind);
        kind.gravity = gravity;
        kind.step_height = step;
    }
    let jump = jump_base * serverkeys_jump_factor(jump_base, base.gravity, x);
    m.extra.vector[0] = jump;
    seen.insert(m.id, SkBase { jump: jump_base, jump_set: jump, ..base });
}

#[derive(Clone, Copy)]
struct SkBase {
    gravity: f64,
    step: f64,
    /// The jump strength the server gave, and the one set from it.
    jump: f64,
    jump_set: f64,
}

fn serverkeys_mount_seen() -> &'static std::sync::Mutex<std::collections::HashMap<u32, SkBase>> {
    static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u32, SkBase>>> = std::sync::OnceLock::new();
    SEEN.get_or_init(Default::default)
}

/// How high a jump of vertical speed `v` goes under `gravity` (`LivingEntity.travel`: the move,
/// then the gravity, then the air's 0.98).
fn serverkeys_peak(mut v: f64, gravity: f64) -> f64 {
    let (mut y, mut top) = (0.0f64, 0.0f64);
    for _ in 0..400 {
        if v <= 0.0 {
            break;
        }
        y += v;
        top = top.max(y);
        v = (v - gravity) * 0.98;
    }
    top
}

/// What the jump strength is multiplied by so a full jump under `x` times the gravity still
/// reaches the height it does under the horse's own.
fn serverkeys_jump_factor(jump: f64, gravity: f64, x: f64) -> f64 {
    if x <= 1.0 || jump <= 0.0 || gravity <= 0.0 {
        return 1.0;
    }
    let target = serverkeys_peak(jump, gravity);
    let (mut lo, mut hi) = (0.0f64, jump * x.sqrt() * 2.0);
    for _ in 0..50 {
        let mid = (lo + hi) / 2.0;
        if serverkeys_peak(mid, gravity * x) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi / jump
}
