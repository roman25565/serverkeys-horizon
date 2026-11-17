// Before each tick of the horse this player drives: progression's horse perks on it.
fn serverkeys_mount(game: &mut crate::game::Game<'_>, i: usize) {
    crate::game::serverkeys_mount::drive(game, i);
}
