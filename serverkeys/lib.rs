//! Server keys ("Клавіші сервера"): one Horizon client mod for every server mod's keys.
//!
//! The keys are listed in [`KEYS`]: `<server mod>.<action>`, a label and a default key. The
//! player rebinds them in Options > Mods > Server keys. A press sends `<server mod>:<action>` on
//! the channel `serverkeys:action`; the server mod with that id acts on it (and checks
//! everything: the client only says which key was pressed). On a server without those mods the
//! message goes nowhere.
//!
//! The server can also send a perk tree (`serverkeys:tree`), which this mod draws as a screen of
//! its own: a starry sky with the perks as round nodes on branches. A click on a node sends
//! `progression:buy:<id>`, and the server answers with the tree as it is now.
//!
//! To add keys for another server mod: add lines to [`KEYS`], rebuild the client
//! (`update-client.bat`), and handle `<mod id>:<action>` in that mod's `net_from_client`.

use horizon_client::mod_api::*;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MOD_ID: &str = "serverkeys";
const CHANNEL: &str = "serverkeys:action";
/// What the hello says: this client draws the tree (protocol 3).
const HELLO: &[u8] = b"hello:3";

/// name (`<server mod>.<action>`), label, default key.
const KEYS: &[(&str, &str, &str)] = &[
    ("progression.perks", "Прогресія: меню перків", "key.keyboard.k"),
    ("progression.dash_forward", "Прогресія: ривок вперед", "key.keyboard.r"),
    ("progression.dash_up", "Прогресія: ривок вгору", "key.keyboard.v"),
    ("progression.spirit", "Прогресія: дух (вийти з тіла / назад)", "key.keyboard.b"),
    ("progression.lab", "Прогресія: лабораторія редстоуну", "key.keyboard.l"),
    ("progression.anchor", "Прогресія: якір чанків (поставити / зняти)", "key.keyboard.h"),
    ("progression.horse", "Прогресія: покликати коня", "key.keyboard.j"),
    ("progression.bags", "Прогресія: сідельні сумки коня", "key.keyboard.n"),
];

/// From the server: a push for the local player (three f32, little-endian).
const PUSH_CHANNEL: &str = "serverkeys:push";
/// From the server: the client-side perks this player owns, comma separated.
const PERKS_CHANNEL: &str = "serverkeys:perks";
/// From the server: the perk tree to draw (see [`Tree::parse`]).
const TREE_CHANNEL: &str = "serverkeys:tree";
/// The client says hello this often, so the server knows it has this mod (and sends the perks).
const HELLO_EVERY: Duration = Duration::from_secs(10);

/// A press waits one frame: if a screen (chat, inventory...) took the same key, it was typing,
/// not a key press for the server.
const STALE: Duration = Duration::from_millis(300);

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(ServerKeys { state: Mutex::new(State::default()), started: Instant::now() })
}

#[derive(Default)]
struct State {
    /// Presses waiting for the next frame: (key name in KEYS, the physical key, when).
    pending: Vec<(&'static str, String, Instant)>,
    /// Physical keys a screen took since the last frame.
    on_screen: Vec<String>,
    /// The last hello the server took.
    hello_at: Option<Instant>,
    /// Client-side perks the server says this player owns (`progression:lava_vision`).
    perks: Vec<String>,
    /// The perk tree the server sent last.
    tree: Option<Tree>,
    /// Messages for the server from the tree screen (clicks), sent on the next frame.
    outgoing: Vec<String>,
    /// Close the tree screen on the next frame (a button opened a server menu).
    close: bool,
}

struct ServerKeys {
    state: Mutex<State>,
    started: Instant,
}

/// One perk on the tree.
#[derive(Clone, Debug, PartialEq)]
struct Node {
    id: String,
    parent: Option<String>,
    /// Where it is: units from the centre (one ring per step out).
    x: f32,
    y: f32,
    icon: String,
    level: u32,
    max: u32,
    /// `max`, `owned`, `buy`, `poor`, `locked` (needs a higher level) or `branch` (a branch's
    /// root, not a perk).
    state: String,
    name: String,
    about: String,
    price: Option<u32>,
    hint: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Tree {
    title: String,
    points: u32,
    /// The progression level, its XP and the XP the next level needs (0: the last level).
    level: u32,
    xp: u32,
    need: u32,
    nodes: Vec<Node>,
}

impl Tree {
    /// Lines of tab-separated fields: `T` title, points, open (1/0) and, from server 2.0, level,
    /// xp, need; `N` id, parent (`-` the
    /// centre), x, y, icon, level, max, state, name, about, price (`-` none), hint. Returns the
    /// tree and whether to open the screen.
    fn parse(text: &str) -> Option<(Tree, bool)> {
        let mut tree = Tree::default();
        let mut open = false;
        let mut header = false;
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.first().copied() {
                Some("T") if f.len() >= 4 => {
                    tree.title = f[1].chars().take(40).collect();
                    tree.points = f[2].parse().ok()?;
                    open = f[3] == "1";
                    if f.len() >= 7 {
                        tree.level = f[4].parse().unwrap_or(0);
                        tree.xp = f[5].parse().unwrap_or(0);
                        tree.need = f[6].parse().unwrap_or(0);
                    }
                    header = true;
                }
                Some("N") if f.len() >= 13 && tree.nodes.len() < 64 => {
                    let x: f32 = f[3].parse().ok()?;
                    let y: f32 = f[4].parse().ok()?;
                    if !x.is_finite() || !y.is_finite() || x.abs() > 50.0 || y.abs() > 50.0 {
                        return None;
                    }
                    tree.nodes.push(Node {
                        id: f[1].to_owned(),
                        parent: (f[2] != "-").then(|| f[2].to_owned()),
                        x,
                        y,
                        icon: f[5].to_owned(),
                        level: f[6].parse().ok()?,
                        max: f[7].parse().ok()?,
                        state: f[8].to_owned(),
                        name: f[9].chars().take(48).collect(),
                        about: f[10].chars().take(1500).collect(),
                        price: f[11].parse().ok(),
                        hint: f[12].chars().take(80).collect(),
                    });
                }
                _ => {}
            }
        }
        header.then_some((tree, open))
    }
}

/// Where the tree's parts are on a screen of this size.
struct Layout {
    cx: f32,
    cy: f32,
    unit: f32,
    radius: f32,
    rect: [f32; 4],
}

const HEADER: f32 = 24.0;
const FOOTER: f32 = 28.0;
const BUTTON_W: f32 = 96.0;
const BUTTONS: usize = 5;
const BUTTON_H: f32 = 18.0;

impl Layout {
    fn new(tree: &Tree, rect: [f32; 4]) -> Layout {
        let [x0, y0, w, h] = rect;
        let max_x = tree.nodes.iter().map(|n| n.x.abs()).fold(1.0f32, f32::max);
        let max_y = tree.nodes.iter().map(|n| n.y.abs()).fold(1.0f32, f32::max);
        let area_h = (h - HEADER - FOOTER).max(40.0);
        let unit = ((w / 2.0 - 30.0) / max_x).min((area_h / 2.0 - 18.0) / max_y).max(8.0);
        let radius = (unit * 0.36).clamp(9.0, 13.0);
        Layout { cx: x0 + w / 2.0, cy: y0 + HEADER + area_h / 2.0, unit, radius, rect }
    }

    fn at(&self, n: &Node) -> [f32; 2] {
        [self.cx + n.x * self.unit, self.cy + n.y * self.unit]
    }

    /// How big a node is drawn: a branch's root is bigger.
    fn size(&self, n: &Node) -> f32 {
        if n.state == "branch" { self.radius + 4.0 } else { self.radius }
    }

    /// The node under `mouse`, if any.
    fn hit<'t>(&self, tree: &'t Tree, mouse: [f32; 2]) -> Option<&'t Node> {
        tree.nodes.iter().find(|n| {
            let [x, y] = self.at(n);
            (x - mouse[0]).hypot(y - mouse[1]) <= self.size(n) + 2.0
        })
    }

    /// The footer's buttons: (rect, label, message to the server).
    fn buttons(&self) -> [([f32; 4], &'static str, &'static str); BUTTONS] {
        let [x0, y0, w, h] = self.rect;
        let y = y0 + h - FOOTER + (FOOTER - BUTTON_H) / 2.0;
        // As wide as fits: five side by side, 6 pixels apart.
        let bw = BUTTON_W.min((w - 16.0) / BUTTONS as f32 - 6.0).max(40.0);
        let left = x0 + w / 2.0 - (bw * BUTTONS as f32 + 6.0 * (BUTTONS as f32 - 1.0)) / 2.0;
        let at = |i: f32| [left + i * (bw + 6.0), y, bw, BUTTON_H];
        [
            (at(0.0), "Вівтар надчар", "progression:altar"),
            (at(1.0), "Лабораторія", "progression:lab"),
            (at(2.0), "Покликати коня", "progression:horse"),
            (at(3.0), "Сідельні сумки", "progression:bags"),
            (at(4.0), "Скинути перки", "progression:respec"),
        ]
    }
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

/// About how wide a line of the game's font is (most letters are 5 pixels and a gap).
fn text_w(s: &str) -> f32 {
    (s.chars().count() as f32 * 6.0 - 1.0).max(0.0)
}

/// Words into lines of at most `max` characters.
fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A colour with its alpha scaled by `a` (0 to 1).
fn alpha(argb: u32, a: f32) -> u32 {
    let base = (argb >> 24) as f32 / 255.0;
    let v = (base * a.clamp(0.0, 1.0) * 255.0).round() as u32;
    (v << 24) | (argb & 0x00FF_FFFF)
}

/// A tiny repeatable random number in 0..1 for star `i`, `salt`.
fn hash(i: u32, salt: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn disc(host: &dyn Host, batch: &mut HudBatch, c: [f32; 2], r: f32, argb: u32) {
    const SEGMENTS: usize = 20;
    let point = |i: usize| {
        let a = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    };
    for i in 0..SEGMENTS {
        let (a, b) = (point(i), point(i + 1));
        host.quad(batch, [c, a, b, b], argb);
    }
}

fn line(host: &dyn Host, batch: &mut HudBatch, a: [f32; 2], b: [f32; 2], width: f32, argb: u32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy);
    if len < 0.5 {
        return;
    }
    let (nx, ny) = (-dy / len * width / 2.0, dx / len * width / 2.0);
    host.quad(batch, [[a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny]], argb);
}

/// Text centred on `x`.
fn text_c(host: &dyn Host, batch: &mut HudBatch, s: &str, x: f32, y: f32, rgb: u32) {
    host.text(batch, s, [(x - text_w(s) / 2.0).round(), y.round()], rgb, true);
}

impl ServerKeys {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn set_perks(&self, perks: Vec<String>) {
        horizon_client::serverkeys_flags::set_lava_swim(perks.iter().any(|p| p == "progression:lava_swim"));
        self.state().perks = perks;
    }

    /// Sends what was pressed while playing, and what the tree screen clicked.
    fn flush(&self, host: &mut dyn Host) {
        let hello_due = self.state().hello_at.is_none_or(|t| t.elapsed() >= HELLO_EVERY);
        if hello_due {
            let sent = host.net_send_to_server(CHANNEL, HELLO).is_ok();
            if sent {
                self.state().hello_at = Some(Instant::now());
            } else {
                // Not on a server that carries mod messages (or not joined yet): no perks here.
                {
                    let mut s = self.state();
                    s.hello_at = None;
                    s.tree = None;
                }
                self.set_perks(Vec::new());
            }
        }
        let (close, outgoing) = {
            let mut s = self.state();
            (std::mem::take(&mut s.close), std::mem::take(&mut s.outgoing))
        };
        for message in outgoing {
            if let Err(e) = host.net_send_to_server(CHANNEL, message.as_bytes()) {
                log::debug!("[serverkeys] {message}: {e}");
            }
        }
        if close {
            host.mod_screen_close(MOD_ID);
        }
        let (pending, on_screen) = {
            let mut s = self.state();
            if s.pending.is_empty() {
                s.on_screen.clear();
                return;
            }
            (std::mem::take(&mut s.pending), std::mem::take(&mut s.on_screen))
        };
        for (name, key, at) in pending {
            if on_screen.contains(&key) || at.elapsed() > STALE {
                continue;
            }
            let message = name.replacen('.', ":", 1);
            if let Err(e) = host.net_send_to_server(CHANNEL, message.as_bytes()) {
                log::debug!("[serverkeys] {message}: {e}");
            }
        }
    }

    fn draw_tree(&self, host: &dyn Host, batch: &mut HudBatch, tree: &Tree, rect: [f32; 4], mouse: [f32; 2]) {
        let t = self.started.elapsed().as_secs_f32();
        let [x0, y0, w, h] = rect;
        let lay = Layout::new(tree, rect);
        // The night sky, a few soft nebulae and twinkling stars.
        host.fill(batch, [x0, y0, w, h], 0xF208_0A18);
        for (i, color) in [0x2A5B_3BC8u32, 0x2238_7BD8, 0x1C9C_3A9E].into_iter().enumerate() {
            let c = [x0 + w * hash(i as u32, 7), y0 + h * hash(i as u32, 8)];
            for k in 0..4 {
                disc(host, batch, c, w.min(h) * (0.18 + 0.07 * k as f32), alpha(color, 0.5));
            }
        }
        let stars = ((w * h) / 900.0).clamp(60.0, 260.0) as u32;
        for i in 0..stars {
            let (sx, sy) = (x0 + w * hash(i, 1), y0 + h * hash(i, 2));
            let twinkle = 0.55 + 0.45 * (t * (0.8 + 2.0 * hash(i, 3)) + 6.3 * hash(i, 4)).sin();
            let size = if hash(i, 5) > 0.92 { 2.0 } else { 1.0 };
            let tint = if hash(i, 6) > 0.8 { 0xFFFF_E9B0 } else { 0xFFDD_E6FF };
            host.fill(batch, [sx.floor(), sy.floor(), size, size], alpha(tint, twinkle));
        }
        // Each branch a constellation: a faint glow of its own colour around its stars.
        const TINTS: [u32; 8] = [0xFF6C_E08A, 0xFFFF_6C6C, 0xFFE0_9A4A, 0xFFE8_D27A, 0xFFFF_4A4A, 0xFF9A_A8C0, 0xFFC8_8A5A, 0xFF8A_C8FF];
        let roots: Vec<&Node> = tree.nodes.iter().filter(|n| n.state == "branch").collect();
        for (b, root) in roots.iter().enumerate() {
            let tint = TINTS[b % TINTS.len()];
            let group: Vec<[f32; 2]> = tree.nodes.iter().filter(|n| Self::root_of(tree, n) == Some(root.id.as_str())).map(|n| lay.at(n)).collect();
            if group.is_empty() {
                continue;
            }
            let (sx, sy) = group.iter().fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
            let mid = [sx / group.len() as f32, sy / group.len() as f32];
            let reach = group.iter().map(|p| (p[0] - mid[0]).hypot(p[1] - mid[1])).fold(lay.radius * 2.0, f32::max);
            for k in 0..3 {
                disc(host, batch, mid, reach * (0.75 + 0.2 * k as f32), alpha(tint, 0.05));
            }
        }
        // Branches.
        let centre = [lay.cx, lay.cy];
        // From server 2.0 every branch has a root of its own: no star in the middle.
        let hubs = tree.nodes.iter().any(|n| n.state == "branch");
        for n in &tree.nodes {
            let parent = n.parent.as_ref().and_then(|p| tree.nodes.iter().find(|m| &m.id == p));
            if parent.is_none() && hubs {
                continue;
            }
            let from = parent.map_or(centre, |m| lay.at(m));
            let lit = n.level > 0 || (n.state == "branch" && tree.nodes.iter().any(|m| m.parent.as_deref() == Some(n.id.as_str()) && m.level > 0));
            let to = lay.at(n);
            // The constellation's lines: thin starlight, brighter where it is learnt.
            if lit {
                line(host, batch, from, to, 4.0, 0x30A8_D8FF);
            }
            line(host, batch, from, to, if lit { 1.5 } else { 1.0 }, if lit { 0xFFCF_E8FF } else { 0x804A_5878 });
        }
        // The centre: the points.
        let pulse = 0.5 + 0.5 * (t * 1.6).sin();
        if !hubs {
            disc(host, batch, centre, lay.radius + 5.0 + pulse * 2.0, 0x30B0_8CFF);
            disc(host, batch, centre, lay.radius + 3.0, 0xFF6C_5CE0);
            disc(host, batch, centre, lay.radius + 1.0, 0xFF16_1430);
            host.item(batch, "minecraft:nether_star", [centre[0] - 8.0, centre[1] - 8.0], 16.0);
        }
        // The perks.
        let hovered = lay.hit(tree, mouse).map(|n| n.id.clone());
        for n in &tree.nodes {
            let c = lay.at(n);
            let r = lay.size(n);
            let (ring, inner) = match n.state.as_str() {
                "branch" => (0xFFB0_8CFF, 0xFF1E_1840),
                "max" => (0xFFFF_C94A, 0xFF2E_2410),
                "owned" => (0xFF5F_C8FF, 0xFF10_2234),
                "buy" => (0xFF7C_FF7C, 0xFF10_2A16),
                _ => (0xFF4A_4E66, 0xFF14_1622),
            };
            // A star: its glow and rays, which twinkle; the learnt ones shine.
            let shine = match n.state.as_str() {
                "max" => 1.0,
                "owned" => 0.8,
                "buy" => 0.6,
                "branch" => 0.7,
                _ => 0.25,
            };
            let tw = 0.75 + 0.25 * (t * (1.3 + hash(c[0].to_bits(), 9)) + hash(c[1].to_bits(), 10) * 6.3).sin();
            let ray = (r + 6.0) * (0.8 + 0.4 * tw) * shine.max(0.5);
            let glint = alpha(0xFFE8_F2FF, 0.55 * shine * tw);
            line(host, batch, [c[0] - ray, c[1]], [c[0] + ray, c[1]], 1.0, glint);
            line(host, batch, [c[0], c[1] - ray], [c[0], c[1] + ray], 1.0, glint);
            disc(host, batch, c, r + 5.0, alpha(0xFFB8_D8FF, 0.10 * shine * tw));
            if n.state == "branch" {
                disc(host, batch, c, r + 4.0 + pulse * 2.0, 0x30B0_8CFF);
            } else if n.state == "buy" {
                let p = 0.5 + 0.5 * (t * 3.0).sin();
                disc(host, batch, c, r + 3.0 + p * 2.0, alpha(ring, 0.25));
            } else if n.state == "max" {
                disc(host, batch, c, r + 3.0, alpha(ring, 0.22));
            }
            if hovered.as_deref() == Some(n.id.as_str()) {
                disc(host, batch, c, r + 2.5, 0xFFFF_FFFF);
            }
            disc(host, batch, c, r + 1.0, ring);
            disc(host, batch, c, r - 1.0, inner);
            host.item(batch, &n.icon, [c[0] - 8.0, c[1] - 8.0], 16.0);
            match n.state.as_str() {
                "poor" => disc(host, batch, c, r - 1.0, 0x8C00_0000),
                "locked" => disc(host, batch, c, r - 1.0, 0xB000_0000),
                "branch" => text_c(host, batch, &n.name, c[0], c[1] + r + 3.0, 0xD8C8_FF),
                _ => {}
            }
            if n.max > 1 {
                let label = format!("{}/{}", n.level, n.max);
                let color = match n.state.as_str() {
                    "max" => 0xFFC9_4A,
                    _ if n.level > 0 => 0x9FE0_FF,
                    _ => 0x8890_A8,
                };
                text_c(host, batch, &label, c[0], c[1] + r + 2.0, color);
            }
        }
        // The header and the footer.
        host.fill(batch, [x0, y0, w, HEADER], 0x9006_0812);
        text_c(host, batch, &format!("« {} »", tree.title), lay.cx, y0 + 8.0, 0xFFD7_6A);
        host.text(batch, &format!("ОП: {}", tree.points), [x0 + 8.0, y0 + 8.0], 0x55FF_FF, true);
        if tree.need > 0 {
            // The XP to the next level, a thin bar under the header.
            let part = (tree.xp as f32 / tree.need as f32).clamp(0.0, 1.0);
            host.fill(batch, [x0, y0 + HEADER - 2.0, w, 2.0], 0xFF20_2038);
            host.fill(batch, [x0, y0 + HEADER - 2.0, w * part, 2.0], 0xFF7C_FF7C);
            let label = format!("{} / {} досвіду", tree.xp, tree.need);
            host.text(batch, &label, [x0 + 8.0 + text_w(&format!("ОП: {}", tree.points)) + 12.0, y0 + 8.0], 0x7CFF_7C, true);
        }
        let tip = "Esc: закрити";
        host.text(batch, tip, [x0 + w - text_w(tip) - 8.0, y0 + 8.0], 0x7780_99, true);
        for (r, label, _) in lay.buttons() {
            let over = inside(r, mouse);
            host.fill(batch, r, if over { 0xFF5A_4FB0 } else { 0xFF2A_2650 });
            host.fill(batch, [r[0], r[1] + r[3] - 1.0, r[2], 1.0], 0xFF8C_7FE8);
            text_c(host, batch, label, r[0] + r[2] / 2.0, r[1] + 5.0, if over { 0xFFFF_A0 } else { 0xE0E0_E0 });
        }
        // The hovered perk's card.
        if let Some(n) = hovered.and_then(|id| tree.nodes.iter().find(|n| n.id == id)) {
            self.draw_card(host, batch, n, rect, mouse);
        }
    }

    /// The branch a node hangs from (its root's id).
    fn root_of<'t>(tree: &'t Tree, n: &'t Node) -> Option<&'t str> {
        let mut at = n;
        for _ in 0..32 {
            if at.state == "branch" {
                return Some(at.id.as_str());
            }
            let p = at.parent.as_deref()?;
            at = tree.nodes.iter().find(|m| m.id == p)?;
        }
        None
    }

    fn draw_card(&self, host: &dyn Host, batch: &mut HudBatch, n: &Node, rect: [f32; 4], mouse: [f32; 2]) {
        let mut lines: Vec<(String, u32)> = vec![(n.name.clone(), 0xFFFF_55)];
        // `¶` starts a line: what it does, then "Зараз:" (green) and "Далі:"/"Дає:" (aqua).
        for part in n.about.split('¶') {
            let color = if part.starts_with("Зараз:") {
                0x55FF_55
            } else if part.starts_with("Далі:") || part.starts_with("Дає:") {
                0x55FF_FF
            } else {
                0xAAAA_AA
            };
            for l in wrap(part, 40) {
                lines.push((l, color));
            }
        }
        if n.state == "branch" {
            lines[0].1 = 0xD8C8_FF;
        } else if n.max > 1 {
            lines.push((format!("Рівень: {} з {}", n.level, n.max), 0xFFFF_FF));
        }
        match (n.state.as_str(), n.price) {
            ("max", _) if n.max > 1 => lines.push(("Максимальний рівень".into(), 0xFFAA_00)),
            ("max", _) => lines.push(("Куплено".into(), 0xFFAA_00)),
            (_, Some(p)) => {
                let label = if n.max > 1 { "Наступний рівень" } else { "Ціна" };
                let color = if n.state == "buy" { 0x55FF_55 } else { 0xFF55_55 };
                lines.push((format!("{label}: {p} ОП"), color));
            }
            _ => {}
        }
        if !n.hint.is_empty() && !(n.state == "max" && n.max > 1) {
            lines.push((n.hint.clone(), 0x55FF_FF));
        }
        let w = lines.iter().map(|(l, _)| text_w(l)).fold(0.0f32, f32::max) + 10.0;
        let h = lines.len() as f32 * 10.0 + 6.0;
        let [x0, y0, rw, rh] = rect;
        let mut x = mouse[0] + 12.0;
        let mut y = mouse[1] - 12.0;
        if x + w > x0 + rw - 4.0 {
            x = mouse[0] - 12.0 - w;
        }
        x = x.max(x0 + 4.0);
        y = y.clamp(y0 + 4.0, (y0 + rh - h - 4.0).max(y0 + 4.0));
        host.fill(batch, [x - 1.0, y - 1.0, w + 2.0, h + 2.0], 0xF050_00FF);
        host.fill(batch, [x, y, w, h], 0xF010_0010);
        for (i, (l, color)) in lines.iter().enumerate() {
            host.text(batch, l, [x + 5.0, y + 4.0 + i as f32 * 10.0], *color, true);
        }
    }
}

impl Hooks for ServerKeys {
    fn mods(&self, host: &mut dyn Host, event: &mut ModsEvent<'_>) {
        let _ = host;
        event.mods.push(NativeMod {
            id: MOD_ID.to_owned(),
            title: "Клавіші сервера 0.7.0".to_owned(),
            description: "Клавіші для модів сервера (перки, навички) і екран перків. Працюють лише на сервері з цими модами."
                .to_owned(),
            keys: KEYS.iter().map(|(n, l, d)| [(*n).to_owned(), (*l).to_owned(), (*d).to_owned()]).collect(),
        });
    }

    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        if !event.pressed || !host.on(MOD_ID) {
            return;
        }
        let pressed: Vec<&'static str> = KEYS
            .iter()
            .filter(|(name, _, default)| host.mod_key(MOD_ID, name, default) == event.key)
            .map(|(name, _, _)| *name)
            .collect();
        if pressed.is_empty() {
            return;
        }
        let now = Instant::now();
        let mut s = self.state();
        for name in pressed {
            s.pending.push((name, event.key.to_owned(), now));
        }
    }

    /// A key a screen (chat, a sign, the inventory) takes is typing, not ours.
    fn screen_key_pressed_pre(&self, host: &dyn Host, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
        let _ = host;
        let mut s = self.state();
        s.on_screen.push(event.key.to_owned());
        if s.on_screen.len() > 64 {
            s.on_screen.remove(0);
        }
        Flow::Continue
    }

    fn net_from_server(&self, host: &mut dyn Host, event: &mut NetFromServerEvent<'_>) {
        match event.channel {
            PUSH_CHANNEL if event.data.len() == 12 => {
                let f = |i: usize| {
                    let b: [u8; 4] = event.data[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4]);
                    f32::from_le_bytes(b)
                };
                host.push_player([f(0), f(1), f(2)]);
            }
            PERKS_CHANNEL => {
                let text = std::str::from_utf8(event.data).unwrap_or("");
                self.set_perks(text.split(',').filter(|p| !p.is_empty()).map(str::to_owned).collect());
            }
            TREE_CHANNEL => {
                let Some((tree, open)) = std::str::from_utf8(event.data).ok().and_then(Tree::parse) else {
                    log::warn!("[serverkeys] the server sent a perk tree this version can't read");
                    return;
                };
                self.state().tree = Some(tree);
                if open && !host.mod_screen_open(MOD_ID) {
                    log::debug!("[serverkeys] the perk screen can't open over the open screen");
                }
            }
            _ => {}
        }
    }

    fn screen_render_post(&self, host: &dyn Host, event: &mut ScreenRenderPostEvent) {
        if host.mod_screen() != Some(MOD_ID) {
            return;
        }
        let tree = self.state().tree.clone();
        if let Some(tree) = tree {
            self.draw_tree(host, event.batch, &tree, event.screen.rect, event.screen.mouse);
        }
    }

    fn screen_mouse_button_pressed_pre(&self, host: &dyn Host, event: &mut ScreenMouseButtonPressedPreEvent) -> Flow {
        if host.mod_screen() != Some(MOD_ID) {
            return Flow::Continue;
        }
        if event.button != 0 {
            return Flow::Cancel;
        }
        let mut s = self.state();
        let Some(tree) = s.tree.as_ref() else {
            return Flow::Cancel;
        };
        let lay = Layout::new(tree, event.screen.rect);
        let mouse = event.screen.mouse;
        let message = if let Some(n) = lay.hit(tree, mouse) {
            (n.state != "branch").then(|| (format!("progression:buy:{}", n.id), false))
        } else {
            lay.buttons().into_iter().find(|(r, _, _)| inside(*r, mouse)).map(|(_, _, m)| (m.to_owned(), true))
        };
        if let Some((message, close)) = message {
            s.outgoing.push(message);
            s.close |= close;
        }
        Flow::Cancel
    }

    /// "Бачення під лавою": in lava the fog is as far as under water with night vision.
    fn render_fog(&self, host: &dyn Host, event: &mut RenderFogEvent<'_>) {
        let _ = host;
        if !matches!(event.fog_type, FogType::Lava) {
            return;
        }
        if !self.state().perks.iter().any(|p| p == "progression:lava_vision") {
            return;
        }
        let end = event.fog.render_distance_end.clamp(16.0, 64.0);
        event.fog.environmental_start = 0.0;
        event.fog.environmental_end = end;
    }

    /// Once a frame, after the key's own event and any screen's: send what was pressed.
    fn frame(&self, host: &mut dyn Host, event: &mut FrameEvent) {
        let _ = event;
        self.flush(host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_parses() {
        let text = "T\tПерки\t12\t1\nN\thealth\t-\t0.000\t-1.000\tminecraft:golden_apple\t1\t5\towned\tЗдоров'я\t+2 серця\t5\tЛКМ: купити рівень\n";
        let (tree, open) = Tree::parse(text).unwrap();
        assert!(open);
        assert_eq!(tree.points, 12);
        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(tree.nodes[0].price, Some(5));
        assert!(Tree::parse("N\tx\n").is_none());
    }

    #[test]
    fn wraps_words() {
        assert_eq!(wrap("один два три", 8), vec!["один два", "три"]);
    }
}
