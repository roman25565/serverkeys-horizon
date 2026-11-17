// Server keys: in lava, with the server's Fire Resistance perk, the local player travels as in
// water (vanilla's water speed, drag and sinking) instead of lava's heavy drag. The fluid state is
// worked out again at the start of the next tick.
fn serverkeys_travel(world: &mut MotionWorld<'_, '_, '_>) {
    if crate::serverkeys_flags::lava_swim()
        && !world.player.flying
        && !world.player.fluids.was_touching_water
        && world.player.is_in_lava()
    {
        world.player.fluids.was_touching_water = true;
    }
}
