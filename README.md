# Server keys (serverkeys)

One Horizon client mod for the keys of all server mods. Players rebind the keys in
Options → Mods → Server keys; each press goes to the server as a `<mod>:<action>` message on the
`serverkeys:action` channel, and the server mod itself checks whether the player is allowed to do it.

Currently: progression (perks K, quests J, night vision N, dash forward R, dash up V).

To add keys for another mod: add a line to `KEYS` (`serverkeys/api/src/lib.rs`), rebuild the client,
and in the server mod handle `<mod id>:<action>` in the `net_from_client` hook.
A vanilla client doesn't have this mod: for it there is the G window ("Quick actions").

## License

MIT, see [LICENSE](LICENSE). A mod for [Horizon](https://github.com/horizon-rust/horizon) (MIT, Ludovic Marie and the Horizon contributors).

Unofficial project, not affiliated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Synergies AB.
