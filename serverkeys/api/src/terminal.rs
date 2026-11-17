//! The Global Storage terminal: the server's storage drawn over its chest menu, with a search
//! box (type to filter, at once), a scrolling grid of icons with their counts, a sort button
//! and a deposit button. The player's inventory below stays the chest's own: a click there
//! puts that stack in, as on any client.
//!
//! The server (`global_storage` mod) sends what the storage holds on `global_storage:term`
//! when the chest opens and then only what changed; the client filters and sorts it itself,
//! so typing costs the server nothing. Clicks on the grid go back as actions on
//! `global_storage:act`. A client without this mod sees the chest's own pages instead.

use horizon_client::mod_api::*;
use std::collections::HashMap;
use std::time::Instant;

pub const TO_CLIENT: &str = "global_storage:term";
pub const TO_SERVER: &str = "global_storage:act";
const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

/// The chest's own menu type (six rows).
const MENU: &str = "minecraft:generic_9x6";
const COLS: usize = 9;
const ROWS: usize = 6;
const MAX_SEARCH: usize = 50;

// Colours (ARGB / RGB).
const PANEL: u32 = 0xFFC6_C6C6;
const PANEL_DARK: u32 = 0xFF55_5555;
const PANEL_LIGHT: u32 = 0xFFFF_FFFF;
const BOX_BORDER: u32 = 0xFFA0_A0A0;
const BOX: u32 = 0xFF00_0000;
const HOVER: u32 = 0x80FF_FFFF;
const BUTTON: u32 = 0xFF8B_8B8B;
const BUTTON_HOVER: u32 = 0xFFA8_A8D8;
const LABEL: u32 = 0x40_4040;
const WHITE: u32 = 0xFF_FFFF;
const GRAY: u32 = 0xA0_A0A0;
const GOLD: u32 = 0xFF_AA00;
const DARK_GRAY: u32 = 0x55_5555;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Count,
    Name,
    Mod,
}

impl Sort {
    fn next(self, back: bool) -> Self {
        match (self, back) {
            (Sort::Count, false) | (Sort::Mod, true) => Sort::Name,
            (Sort::Name, false) | (Sort::Count, true) => Sort::Mod,
            (Sort::Mod, false) | (Sort::Name, true) => Sort::Count,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Sort::Count => "К-сть",
            Sort::Name => "А-Я",
            Sort::Mod => "Мод",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Sort::Count => "за кількістю",
            Sort::Name => "за назвою (А-Я)",
            Sort::Mod => "за модом",
        }
    }
}

struct Entry {
    /// `minecraft:diamond`.
    id: String,
    name: String,
    /// Lowercase: id, names, the stack's own name.
    hay: String,
    stack: ItemStack,
    count: u64,
}

/// The open terminal.
#[derive(Default)]
pub struct Terminal {
    entries: HashMap<u32, Entry>,
    search: String,
    sort: Sort,
    /// The first row shown.
    scroll: usize,
    /// The entries shown, in order; None when it must be worked out again.
    shown: Option<Vec<u32>>,
    opened: Option<Instant>,
    /// The chest's title, as the server gave it: the terminal draws over that chest only.
    title: String,
}

/// Where things are, from the chest panel's rect `[x, y, w, h]`.
struct Layout {
    x: f32,
    y: f32,
}

impl Layout {
    fn new(rect: [f32; 4]) -> Self {
        Layout { x: rect[0], y: rect[1] }
    }
    fn search(&self) -> [f32; 4] {
        [self.x + 7.0, self.y + 4.0, 128.0, 12.0]
    }
    fn sort(&self) -> [f32; 4] {
        [self.x + 137.0, self.y + 4.0, 32.0, 12.0]
    }
    fn slot(&self, i: usize) -> [f32; 2] {
        [self.x + 8.0 + (i % COLS) as f32 * 18.0, self.y + 18.0 + (i / COLS) as f32 * 18.0]
    }
    fn grid(&self) -> [f32; 4] {
        [self.x + 7.0, self.y + 17.0, COLS as f32 * 18.0, ROWS as f32 * 18.0]
    }
    fn status(&self) -> [f32; 4] {
        [self.x + 7.0, self.y + 126.0, 162.0, 12.0]
    }
    fn deposit(&self) -> [f32; 4] {
        [self.x + 121.0, self.y + 126.0, 48.0, 12.0]
    }
    fn scrollbar(&self) -> [f32; 4] {
        [self.x + 177.0, self.y + 17.0, 10.0, ROWS as f32 * 18.0]
    }
    /// The whole area the terminal answers for (above the inventory).
    fn top(&self) -> [f32; 4] {
        [self.x, self.y, 190.0, 138.0]
    }
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

/// `12 345`.
fn group(n: u64) -> String {
    let d = n.to_string();
    let mut out = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// A count in four characters at most: 999, 1.2k, 12k, 123k, 1.2M...
pub fn short(n: u64) -> String {
    const UNITS: [(u64, &str); 4] = [
        (1_000_000_000_000, "T"),
        (1_000_000_000, "B"),
        (1_000_000, "M"),
        (1_000, "k"),
    ];
    if n < 1000 {
        return n.to_string();
    }
    if n >= 999_000_000_000_000 {
        return "999T".into();
    }
    for (u, s) in UNITS {
        if n >= u {
            let v = n as f64 / u as f64;
            return if v < 10.0 {
                let t = (v * 10.0).floor() / 10.0;
                format!("{t:.1}{s}")
            } else {
                format!("{}{s}", v.floor() as u64)
            };
        }
    }
    n.to_string()
}

/// Whether an entry matches the search: every word in its text, `@mod` its namespace.
fn matches(e: &Entry, words: &[String]) -> bool {
    let ns = e.id.split_once(':').map_or("minecraft", |(n, _)| n);
    words.iter().all(|w| match w.strip_prefix('@') {
        Some(m) => ns.starts_with(m),
        None => e.hay.contains(w.as_str()),
    })
}

impl Terminal {
    fn words(&self) -> Vec<String> {
        self.search
            .to_lowercase()
            .replace(['’', 'ʼ', '`'], "'")
            .split_whitespace()
            .filter(|w| *w != "@")
            .map(str::to_owned)
            .collect()
    }

    fn shown(&mut self) -> &[u32] {
        if self.shown.is_none() {
            let words = self.words();
            let mut ids: Vec<u32> = self
                .entries
                .iter()
                .filter(|(_, e)| matches(e, &words))
                .map(|(id, _)| *id)
                .collect();
            let e = &self.entries;
            let by_name = |a: &u32, b: &u32| {
                e[a].name
                    .to_lowercase()
                    .cmp(&e[b].name.to_lowercase())
                    .then(a.cmp(b))
            };
            match self.sort {
                Sort::Count => ids.sort_by(|a, b| e[b].count.cmp(&e[a].count).then_with(|| by_name(a, b))),
                Sort::Name => ids.sort_by(by_name),
                Sort::Mod => ids.sort_by(|a, b| e[a].id.cmp(&e[b].id).then_with(|| by_name(a, b))),
            }
            self.shown = Some(ids);
        }
        self.shown.as_deref().unwrap_or(&[])
    }

    fn rows(&mut self) -> usize {
        self.shown().len().div_ceil(COLS)
    }

    fn max_scroll(&mut self) -> usize {
        self.rows().saturating_sub(ROWS)
    }

    fn scroll_by(&mut self, rows: i64) {
        let max = self.max_scroll() as i64;
        self.scroll = (self.scroll as i64 + rows).clamp(0, max) as usize;
    }

    /// The entry under grid cell `cell` (0..54).
    fn at(&mut self, cell: usize) -> Option<u32> {
        let i = self.scroll * COLS + cell;
        self.shown().get(i).copied()
    }

    /// Records from the server. Returns false on `close`.
    pub fn apply(&mut self, text: &str) -> bool {
        for record in text.split(RECORD) {
            let f: Vec<&str> = record.split(FIELD).collect();
            match f.as_slice() {
                ["open", title, ..] => {
                    *self = Terminal {
                        sort: self.sort,
                        opened: Some(Instant::now()),
                        title: (*title).to_owned(),
                        ..Terminal::default()
                    };
                }
                ["add", id, count, name, hay, snbt] => {
                    let (Ok(id), Ok(count)) = (id.parse(), count.parse()) else {
                        continue;
                    };
                    let item = hay.split(' ').next().unwrap_or("minecraft:barrier").to_owned();
                    let stack = horizon_client::item_data::parse_stack(snbt)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| ItemStack::new(&item, 1));
                    self.entries.insert(
                        id,
                        Entry {
                            id: item,
                            name: (*name).to_owned(),
                            hay: hay.to_lowercase(),
                            stack,
                            count,
                        },
                    );
                }
                ["set", id, count] => {
                    if let (Ok(id), Ok(count)) = (id.parse::<u32>(), count.parse())
                        && let Some(e) = self.entries.get_mut(&id)
                    {
                        e.count = count;
                    }
                }
                ["del", id] => {
                    if let Ok(id) = id.parse::<u32>() {
                        self.entries.remove(&id);
                    }
                }
                ["close"] => return false,
                _ => {}
            }
        }
        self.shown = None;
        let max = self.max_scroll();
        self.scroll = self.scroll.min(max);
        true
    }

    /// The terminal belongs on this screen: the chest the server opened for it.
    pub fn on(&self, screen: &NativeScreenView) -> bool {
        screen.menu == Some(MENU) && screen.title.is_some_and(|t| t.plain() == self.title)
    }

    pub fn draw(&mut self, host: &dyn Host, batch: &mut HudBatch, screen: &NativeScreenView) {
        let l = Layout::new(screen.rect);
        let mouse = screen.mouse;
        // The chest's title row and the inventory's label row: our own there.
        host.fill(batch, [l.x + 4.0, l.y + 3.0, 168.0, 14.0], PANEL);
        host.fill(batch, l.status(), PANEL);

        // The search box.
        let r = l.search();
        host.fill(batch, r, BOX_BORDER);
        host.fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], BOX);
        let blink = self.opened.is_none_or(|t| t.elapsed().as_millis() / 500 % 2 == 0);
        if self.search.is_empty() {
            host.text(batch, "Пошук…", [r[0] + 3.0, r[1] + 2.0], GRAY, false);
            if blink {
                host.text(batch, "_", [r[0] + 3.0, r[1] + 2.0], WHITE, false);
            }
        } else {
            // The end of the text when it is longer than the box.
            let mut shown: String = self.search.clone();
            while host.text_width(&shown) > r[2] - 12.0 && !shown.is_empty() {
                shown.remove(0);
            }
            host.text(batch, &shown, [r[0] + 3.0, r[1] + 2.0], WHITE, false);
            if blink {
                let w = host.text_width(&shown);
                host.text(batch, "_", [r[0] + 3.0 + w, r[1] + 2.0], WHITE, false);
            }
        }

        // The sort button.
        let r = l.sort();
        let hot = inside(r, mouse);
        host.fill(batch, r, if hot { PANEL_LIGHT } else { PANEL_DARK });
        host.fill(
            batch,
            [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
            if hot { BUTTON_HOVER } else { BUTTON },
        );
        let label = self.sort.label();
        let w = host.text_width(label);
        host.text(batch, label, [r[0] + (r[2] - w) / 2.0, r[1] + 2.0], WHITE, true);

        // The grid.
        let total_kinds = self.entries.len();
        let total: u128 = self.entries.values().map(|e| u128::from(e.count)).sum();
        let shown_len = self.shown().len();
        let mut hovered = None;
        for cell in 0..COLS * ROWS {
            let Some(id) = self.at(cell) else {
                break;
            };
            let [sx, sy] = l.slot(cell);
            let e = &self.entries[&id];
            host.item_stack(batch, &e.stack, [sx, sy], 16.0);
            let label = short(e.count);
            let w = host.text_width(&label);
            host.text(batch, &label, [sx + 17.0 - w, sy + 9.0], WHITE, true);
            if inside([sx - 1.0, sy - 1.0, 18.0, 18.0], mouse) {
                host.fill(batch, [sx, sy, 16.0, 16.0], HOVER);
                hovered = Some(id);
            }
        }
        if shown_len == 0 {
            let msg = if self.entries.is_empty() {
                "Склад порожній: клацни свій предмет унизу"
            } else {
                "Нічого не знайдено"
            };
            let g = l.grid();
            let w = host.text_width(msg);
            host.text(batch, msg, [g[0] + (g[2] - w) / 2.0, g[1] + 46.0], LABEL, false);
        }

        // The scrollbar.
        let r = l.scrollbar();
        host.fill(batch, [r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], PANEL_DARK);
        host.fill(batch, r, 0xFF37_3737);
        let max = self.max_scroll();
        let thumb_h = if max == 0 { r[3] } else { (r[3] * ROWS as f32 / (max + ROWS) as f32).max(10.0) };
        let thumb_y = if max == 0 { r[1] } else { r[1] + (r[3] - thumb_h) * self.scroll as f32 / max as f32 };
        host.fill(batch, [r[0], thumb_y, r[2], thumb_h], if max == 0 { BUTTON } else { PANEL });

        // The status line and the deposit button.
        let st = l.status();
        let status = format!("{} видів · {}", group(total_kinds as u64), short(total.min(u128::from(u64::MAX)) as u64));
        host.text(batch, &status, [st[0] + 1.0, st[1] + 2.0], LABEL, false);
        let r = l.deposit();
        let hot_deposit = inside(r, mouse);
        host.fill(batch, r, if hot_deposit { PANEL_LIGHT } else { PANEL_DARK });
        host.fill(
            batch,
            [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
            if hot_deposit { BUTTON_HOVER } else { BUTTON },
        );
        let label = "Покласти";
        let w = host.text_width(label);
        host.text(batch, label, [r[0] + (r[2] - w) / 2.0, r[1] + 2.0], WHITE, true);

        // Tooltips, last so they lie over everything.
        let at = [mouse[0] + 8.0, mouse[1] - 12.0];
        if let Some(id) = hovered {
            let e = &self.entries[&id];
            let mut lines = vec![
                Text::literal(e.name.clone()).with_color(WHITE),
                Text::literal(format!("У сховищі: {}", group(e.count))).with_color(GOLD),
            ];
            let max = host.max_stack_size(&e.id).unwrap_or(64).max(1);
            if max > 1 && e.count >= u64::from(max) {
                let (full, rest) = (e.count / u64::from(max), e.count % u64::from(max));
                let stacks = if rest == 0 {
                    format!("{} ст.", group(full))
                } else {
                    format!("{} ст. + {rest}", group(full))
                };
                lines.push(Text::literal(stacks).with_color(GRAY));
            }
            lines.push(Text::literal("ЛКМ — стак · ПКМ — 1 шт.").with_color(DARK_GRAY));
            lines.push(Text::literal("Shift+клік — скільки влізе").with_color(DARK_GRAY));
            host.gui_tooltip(batch, &lines, at);
        } else if hot {
            let lines = [
                Text::literal("Сортування").with_color(WHITE),
                Text::literal(self.sort.title()).with_color(GOLD),
                Text::literal("ЛКМ — далі · ПКМ — назад").with_color(DARK_GRAY),
            ];
            host.gui_tooltip(batch, &lines, at);
        } else if hot_deposit {
            let lines = [
                Text::literal("Покласти на склад").with_color(WHITE),
                Text::literal("ЛКМ — усе з інвентаря").with_color(GRAY),
                Text::literal("ПКМ — лише те, що вже є на складі").with_color(GRAY),
                Text::literal("Або клік / Shift+клік по предмету внизу").with_color(DARK_GRAY),
            ];
            host.gui_tooltip(batch, &lines, at);
        } else if inside(l.search(), mouse) {
            let lines = [
                Text::literal("Просто друкуй: назва українською чи англійською").with_color(WHITE),
                Text::literal("@мод — лише предмети мода (@create)").with_color(GRAY),
                Text::literal("ПКМ або Delete — очистити").with_color(DARK_GRAY),
            ];
            host.gui_tooltip(batch, &lines, at);
        }
    }

    /// A mouse button on the screen: the action to send, and whether the click was ours.
    pub fn click(&mut self, screen: &NativeScreenView, button: i32, modifiers: u32) -> (Option<String>, bool) {
        let l = Layout::new(screen.rect);
        let m = screen.mouse;
        let shift = modifiers & 1 != 0;
        if inside(l.grid(), m) {
            let col = ((m[0] - l.grid()[0]) / 18.0) as usize;
            let row = ((m[1] - l.grid()[1]) / 18.0) as usize;
            if col < COLS && row < ROWS
                && let Some(id) = self.at(row * COLS + col)
            {
                let how = match (button, shift) {
                    (_, true) => "fill",
                    (1, false) => "one",
                    _ => "stack",
                };
                return (Some(format!("take{FIELD}{id}{FIELD}{how}")), true);
            }
            return (None, true);
        }
        if inside(l.sort(), m) {
            self.sort = self.sort.next(button == 1);
            self.shown = None;
            self.scroll = 0;
            return (None, true);
        }
        if inside(l.deposit(), m) {
            let which = if button == 1 { "known" } else { "all" };
            return (Some(format!("deposit{FIELD}{which}")), true);
        }
        if inside(l.search(), m) {
            if button == 1 {
                self.set_search(String::new());
            }
            return (None, true);
        }
        let r = l.scrollbar();
        if inside([r[0] - 1.0, r[1], r[2] + 2.0, r[3]], m) {
            let max = self.max_scroll();
            self.scroll = (((m[1] - r[1]) / r[3]) * (max + 1) as f32).clamp(0.0, max as f32) as usize;
            return (None, true);
        }
        (None, inside(l.top(), m) && m[1] < l.status()[1] + l.status()[3])
    }

    fn set_search(&mut self, s: String) {
        self.search = s;
        self.shown = None;
        self.scroll = 0;
    }

    pub fn typed(&mut self, c: char) {
        if !c.is_control() && self.search.chars().count() < MAX_SEARCH {
            let mut s = self.search.clone();
            s.push(c);
            self.set_search(s);
        }
    }

    /// A key while the terminal is open: whether it was ours (all but Escape are, so the
    /// inventory and hotbar keys type instead of acting).
    pub fn key(&mut self, key: &str, modifiers: u32) -> bool {
        let control = modifiers & 2 != 0;
        match key {
            "key.keyboard.escape" => return false,
            "key.keyboard.backspace" => {
                let mut s = self.search.clone();
                if control {
                    s.clear();
                } else {
                    s.pop();
                }
                self.set_search(s);
            }
            "key.keyboard.delete" => self.set_search(String::new()),
            "key.keyboard.up" => self.scroll_by(-1),
            "key.keyboard.down" => self.scroll_by(1),
            "key.keyboard.page.up" => self.scroll_by(-(ROWS as i64)),
            "key.keyboard.page.down" => self.scroll_by(ROWS as i64),
            "key.keyboard.home" => self.scroll = 0,
            "key.keyboard.end" => self.scroll = self.max_scroll(),
            _ => {}
        }
        true
    }

    /// The wheel over the terminal: whether it was ours.
    pub fn wheel(&mut self, screen: &NativeScreenView, dy: f64) -> bool {
        let l = Layout::new(screen.rect);
        if !inside(l.top(), screen.mouse) {
            return false;
        }
        let rows = if dy > 0.0 { -1 } else if dy < 0.0 { 1 } else { 0 };
        self.scroll_by(rows);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_counts_fit() {
        assert_eq!(short(0), "0");
        assert_eq!(short(999), "999");
        assert_eq!(short(1000), "1.0k");
        assert_eq!(short(1299), "1.2k");
        assert_eq!(short(12_345), "12k");
        assert_eq!(short(123_456), "123k");
        assert_eq!(short(1_234_567), "1.2M");
        assert_eq!(short(9_999_999_999), "9.9B");
        for n in [0, 5, 999, 1000, 9999, 99_999, 999_999, 1_000_000, u64::MAX] {
            assert!(short(n).chars().count() <= 4, "{n}: {}", short(n));
        }
    }
}
