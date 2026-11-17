// Server keys: switches the trait mod (`horizon_client::serverkeys_flags`) sets from the perks
// the server sends, read by the movement injection.
use std::sync::atomic::{AtomicBool, Ordering};

/// The player swims in lava as in water (progression's Fire Resistance).
static LAVA_SWIM: AtomicBool = AtomicBool::new(false);

pub fn set_lava_swim(on: bool) {
    LAVA_SWIM.store(on, Ordering::Relaxed);
}

pub fn lava_swim() -> bool {
    LAVA_SWIM.load(Ordering::Relaxed)
}

/// "Впевнений спуск" (progression): how many times the gravity of the horse this player drives
/// (1: as vanilla), as f32 bits.
static HORSE_GRAVITY: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000);
/// "Прохідливість" (progression): blocks added to the step of the horse this player drives.
static HORSE_STEP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_horse(gravity: f32, step: f32) {
    HORSE_GRAVITY.store(gravity.max(1.0).to_bits(), Ordering::Relaxed);
    HORSE_STEP.store(step.max(0.0).to_bits(), Ordering::Relaxed);
}

pub fn horse_gravity() -> f32 {
    f32::from_bits(HORSE_GRAVITY.load(Ordering::Relaxed))
}

pub fn horse_step() -> f32 {
    f32::from_bits(HORSE_STEP.load(Ordering::Relaxed))
}
