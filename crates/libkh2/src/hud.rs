//! The field HUD as a 2D draw list (round 10): the command menu (bottom left, with its pages for
//! Magic, Items and Drive) and the reaction capsule, drawn from the game's own art
//! (`field2d/us/<world>0command.2dd`) and fonts (`msg/us/fontinfo.bar`, `fontimage.bar`, the
//! system messages), on the game's 512 x 448 virtual screen.
//!
//! Compiled from kh2-rs source (`kh2_enemy_host::hud_menu`: `shared/hud.rs`'s menu records and
//! `kh2-hud/src/hud_menu.rs`'s group tables read from the executable, the layout constants
//! `page_x` / `row_y` / `header_y` / `capsule_y`, `bar_group`, `icon_group`, `base_entries`) and
//! kh2-rs's pure `kh2_image` crate (the SEQD sequence player and its quads, the text engine).
//! This file is the glue of kh2-rs's Bevy systems `hud_menu::update` and `hud_gfx::HudScene`
//! (nodes, players, quads) without Bevy, and the menu's input.
//!
//! decided: the menu's input is the host's (`input`: up / down / confirm / back); confirming a
//! basic command opens its page (Magic: the spells at their levels, Items: the host's list, Drive:
//! the owned forms) or reports the choice; the gauges, party panels, target gauge, lock-on reticle,
//! banners and the INFORMATION bar are not drawn yet (TODO.md).

use kh2_enemy_host::hud_menu::{self as hm, CommandMenu, Label, MenuEntry, MenuPage, Reaction, Tables};
use kh2_image::font::{Atlas, Font, FontSet};
use kh2_image::seqd::{Ext, Player, Quad, Seqd};
use kh2_image::text::{self, ColorTable, Kind, Messages, Style};
use kh2_tables::tables::Cmd;

use crate::data::Disc;

/// The game's screen in HUD units.
pub const SCREEN: [f32; 2] = [512.0, 448.0];

/// An image the draw list samples (RGBA8, row-major from the top, straight alpha; colours as the
/// PS2 texels have them, so vertex colour 0x80 is 1.0).
#[derive(Clone, Debug, Default)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One quad of the draw list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    /// Index into `Hud::textures`.
    pub texture: u32,
    /// Corners: top-left, top-right, bottom-left, bottom-right, HUD units (y down).
    pub pos: [[f32; 2]; 4],
    /// Texel coordinates of the corners.
    pub uv: [[f32; 2]; 4],
    /// Vertex colours (0x80 = 1.0 for RGB, 0x80 = opaque for alpha).
    pub color: [[u8; 4]; 4],
    /// 0 alpha, 1 additive, 2 subtractive.
    pub blend: u32,
    /// Sample with nearest filtering.
    pub nearest: bool,
    /// True for a text glyph or icon.
    pub text: bool,
}

struct SeqFile {
    texture: u32,
    seq: Seqd,
}

struct Node {
    file: usize,
    player: Player,
    pos: [f32; 2],
    visible: bool,
    label: Option<Label>,
    label_time: f32,
}

impl Node {
    fn new(file: usize) -> Node {
        Node { file, player: Player::default(), pos: [0.0; 2], visible: false, label: None, label_time: 0.0 }
    }
}

/// Fonts and messages (kh2-rs `hud_text::TextSys` without the scene files).
struct TextSys {
    set: FontSet,
    messages: Option<Messages>,
    world_messages: Option<Messages>,
    colors: Option<ColorTable>,
    /// Texture by (atlas, palette row), and the icon atlas.
    pages: Vec<((u8, usize), u32)>,
    icons: u32,
    world: usize,
}

impl TextSys {
    fn lookup(&self, id: u32) -> Option<Vec<text::Token>> {
        let tables = [self.messages.as_ref(), self.world_messages.as_ref()];
        tables.iter().flatten().find_map(|m| m.tokens(id)).or_else(|| self.messages.as_ref()?.tokens(0x8adc))
    }

    fn quads(&self, label: &Label, origin: [f32; 2], time: f32) -> Vec<(u32, Quad)> {
        let font = Font::get(label.style.font);
        let at = [origin[0] + label.offset[0], origin[1] + label.offset[1]];
        let style = Style { time, ..label.style.clone() };
        let l = text::layout(&self.set, self.colors.as_ref(), &label.tokens, &style, at);
        let mut all = text::shadows(&l.quads, font.mode, label.style.color[3]);
        all.extend(l.quads.iter().copied());
        let key = |a: Atlas| match a {
            Atlas::Sys => 0u8,
            Atlas::Evt => 1,
        };
        all.iter()
            .filter_map(|q| {
                let tex = match q.kind {
                    Kind::Glyph(page) => self.pages.iter().find(|(k, _)| *k == (key(font.atlas), font.clut_row() + page as usize))?.1,
                    Kind::Icon => self.icons,
                };
                let [x0, y0, x1, y1] = q.pos;
                let [u0, v0, u1, v1] = q.uv;
                let k = q.skew;
                Some((tex, Quad { pos: [[x0 + k, y0], [x1 + k, y0], [x0 - k, y1], [x1 - k, y1]], uv: [[u0, v0], [u1, v0], [u0, v1], [u1, v1]], color: [q.color; 4], blend: 0, nearest: false }))
            })
            .collect()
    }
}

/// What the player chose in the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The Attack command (command id 1's row).
    Attack,
    /// A spell of magic slot `slot` (the library starts the cast).
    Magic { slot: u8 },
    /// An item of the host's list (the host uses it: `kh2_use_item`).
    Item { id: u16 },
    /// A drive form (the host enters it: `kh2_drive_enter`).
    Drive { form: u8 },
    /// The offered reaction command (the library accepts it).
    Reaction { cmd: u16 },
}

/// The menu's input of one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuInput {
    pub up: bool,
    pub down: bool,
    pub confirm: bool,
    pub back: bool,
    /// The reaction button (triangle).
    pub reaction: bool,
}

/// What a row of a page stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Attack,
    OpenMagic,
    OpenItems,
    OpenDrive,
    Magic(u8),
    Item(u16),
    Drive(u8),
    None,
}

/// The HUD of one Sora.
pub struct Hud {
    pub textures: Vec<Texture>,
    files: Vec<SeqFile>,
    nodes: Vec<Node>,
    text: Option<TextSys>,
    tables: Option<Tables>,
    cmds: Vec<Cmd>,
    menu_file: Option<usize>,
    header: [usize; hm::PAGES],
    rows: [[usize; hm::ROWS]; hm::PAGES],
    selected: [[usize; 4]; hm::PAGES],
    capsule: usize,
    pub menu: CommandMenu,
    actions: Vec<Vec<Action>>,
    /// The host's item list for the Items page (item id, count).
    pub items: Vec<(u16, u8)>,
    /// Choices made, for the host (`kh2_hud_poll_event`; the last 64 kept).
    pub choices: Vec<Choice>,
    /// Choices not yet carried out by the library (`kh2_hud_tick` takes them).
    pub fresh: Vec<Choice>,
    /// The reaction command offered (command id), mirrored into the capsule.
    pub reaction_cmd: Option<u16>,
}

impl Hud {
    /// Loads the HUD's art, fonts and tables for world `world` (two letters; `zz` is the fallback
    /// art kh2-rs uses).
    pub fn load(disc: &mut Disc, world: &str) -> Result<Hud, String> {
        let exe = disc.exe().ok_or("no executable")?;
        let tables = Tables::from_exe(&exe);
        use kh2_tables::tables as t;
        let cmds = kh2_tables::load::<t::Cmd>(&mut disc.0, &t::CMD).map_err(|e| format!("cmd: {e}"))?.rows;
        let mut hud = Hud {
            textures: Vec::new(),
            files: Vec::new(),
            nodes: Vec::new(),
            text: None,
            tables,
            cmds,
            menu_file: None,
            header: [0; hm::PAGES],
            rows: [[0; hm::ROWS]; hm::PAGES],
            selected: [[0; 4]; hm::PAGES],
            capsule: 0,
            menu: CommandMenu::default(),
            actions: Vec::new(),
            items: Vec::new(),
            choices: Vec::new(),
            fresh: Vec::new(),
            reaction_cmd: None,
        };
        let pair = |b: &[u8]| -> Option<(kh2_image::imgd::Image, Seqd)> {
            let e = kh2_bar::parse(b).ok()?;
            e.windows(2).find_map(|w| Some((kh2_image::imgd::decode(w[0].data)?, kh2_image::seqd::parse(w[1].data)?)))
        };
        let art = disc.read(&format!("field2d/us/{world}0command.2dd")).ok().and_then(|b| pair(&b)).or_else(|| disc.read("field2d/us/zz0command.2dd").ok().and_then(|b| pair(&b)));
        if let Some((img, seq)) = art {
            hud.textures.push(Texture { width: img.width, height: img.height, rgba: img.rgba });
            hud.files.push(SeqFile { texture: (hud.textures.len() - 1) as u32, seq });
            hud.menu_file = Some(hud.files.len() - 1);
        }
        // Fonts and messages (kh2-rs `TextSys::load`).
        if let (Ok(info), Ok(image)) = (disc.read("msg/us/fontinfo.bar"), disc.read("msg/us/fontimage.bar"))
            && let Some(set) = FontSet::parse(&info, &image)
        {
            let messages = disc.read("msg/us/sys.bar").ok().and_then(|b| Messages::from_bar(&b));
            let world_messages = disc.read(&format!("msg/us/{world}.bar")).ok().and_then(|b| Messages::from_bar(&b));
            let colors = disc.read("03system.bin").ok().and_then(|b| {
                let e = kh2_bar::parse(&b).ok()?;
                ColorTable::parse(e.iter().find(|e| e.tag == "ftst")?.data)
            });
            let mut pages = Vec::new();
            for id in 0..6 {
                let f = Font::get(id);
                let key = (if matches!(f.atlas, Atlas::Sys) { 0u8 } else { 1 }, f.clut_row());
                if pages.iter().any(|(k, _)| *k == key) {
                    continue;
                }
                for page in 0..2 {
                    let k = (key.0, key.1 + page);
                    let (w, h, rgba) = set.atlas_rgba_ps2(f.atlas, key.1 + page);
                    hud.textures.push(Texture { width: w as u32, height: h as u32, rgba });
                    pages.push((k, (hud.textures.len() - 1) as u32));
                }
            }
            let (w, h, rgba) = set.icons_rgba_ps2();
            hud.textures.push(Texture { width: w as u32, height: h as u32, rgba });
            let icons = (hud.textures.len() - 1) as u32;
            let world_ix = text::WORLDS.iter().position(|w| *w == world).unwrap_or(0);
            hud.text = Some(TextSys { set, messages, world_messages, colors, pages, icons, world: world_ix });
        }
        // The node pool (kh2-rs `hud_menu::build`): per page the header, the plain bars, then the selected bar with its
        // medallion, icon and arrow; the capsule last.
        if let Some(f) = hud.menu_file {
            let add = |nodes: &mut Vec<Node>| {
                nodes.push(Node::new(f));
                nodes.len() - 1
            };
            for p in 0..hm::PAGES {
                hud.header[p] = add(&mut hud.nodes);
                for r in 0..hm::ROWS {
                    hud.rows[p][r] = add(&mut hud.nodes);
                }
                hud.selected[p] = [add(&mut hud.nodes), add(&mut hud.nodes), add(&mut hud.nodes), add(&mut hud.nodes)];
            }
            hud.capsule = add(&mut hud.nodes);
        }
        let base = hm::base_entries(&hud.cmds);
        let actions = [Action::Attack, Action::OpenMagic, Action::OpenItems, Action::OpenDrive];
        hud.menu = CommandMenu { visible: true, pages: vec![MenuPage { kind: 0, entries: base.clone(), cursor: 0 }], reaction: None };
        hud.actions = vec![actions.iter().copied().take(base.len()).collect()];
        Ok(hud)
    }

    fn cmd_entry(&self, id: u16, style: u8) -> Option<MenuEntry> {
        let c = self.cmds.iter().find(|c| c.id == id)?;
        let mut e = hm::entry(c);
        e.style = style;
        Some(e)
    }

    /// Opens a sub-page (Magic: the spells at their levels, MP-short ones orange; Items: the host's
    /// list; Drive: the owned forms).
    fn open(&mut self, kind: u8, sora: &crate::sora::Sora) {
        let mut entries = Vec::new();
        let mut actions = Vec::new();
        match kind {
            1 => {
                for slot in 0..6u8 {
                    if let Some((row, cost)) = sora.spell(slot)
                        && let Some(e) = self.cmd_entry(row.command, if sora.mp < cost as i32 { 2 } else { 0 })
                    {
                        entries.push(e);
                        actions.push(Action::Magic(slot));
                    }
                }
            }
            2 => {
                for &(id, count) in &self.items {
                    if count == 0 {
                        continue;
                    }
                    let msg = sora.game.params.items.iter().find(|i| i.id == id).map(|i| i.name as u32);
                    entries.push(MenuEntry { message: msg.unwrap_or(0x8adc), icon: 0, style: 0 });
                    actions.push(Action::Item(id));
                }
            }
            3 => {
                // The forms whose item is owned, by their drive command (kh2-rs `player::drive_forms`: item.Command).
                let forms: Vec<(u8, u16, u8)> = sora.game.drive.as_ref().map(|d| d.forms.iter().map(|f| (f.form, f.command, f.cost)).collect()).unwrap_or_default();
                for (form, cmd, cost) in forms {
                    if sora.drive.owned.get(form as usize).copied().unwrap_or(false)
                        && let Some(e) = self.cmd_entry(cmd, if sora.drive.points.whole < cost { 5 } else { 0 })
                    {
                        entries.push(e);
                        actions.push(Action::Drive(form));
                    }
                }
            }
            _ => {}
        }
        if entries.is_empty() {
            entries.push(self.cmd_entry(0x68, 5).unwrap_or_default());
            actions.push(Action::None);
        }
        self.menu.pages.push(MenuPage { kind, entries, cursor: 0 });
        self.actions.push(actions);
    }

    /// One tick of menu input (the game's command menu: up / down move the cursor of the last page,
    /// confirm opens a page or chooses, back closes the last sub-page).
    pub fn input(&mut self, i: MenuInput, sora: &crate::sora::Sora) {
        if i.reaction
            && let Some(cmd) = self.reaction_cmd
        {
            self.choose(Choice::Reaction { cmd });
        }
        let pages = self.menu.pages.len();
        let Some(page) = self.menu.pages.last_mut() else { return };
        let n = page.entries.len().max(1);
        if i.up {
            page.cursor = (page.cursor + n - 1) % n;
        }
        if i.down {
            page.cursor = (page.cursor + 1) % n;
        }
        let cursor = page.cursor;
        if i.back && pages > 1 {
            self.menu.pages.pop();
            self.actions.pop();
            return;
        }
        if !i.confirm {
            return;
        }
        let action = self.actions.last().and_then(|a| a.get(cursor)).copied().unwrap_or(Action::None);
        match action {
            Action::Attack => self.choose(Choice::Attack),
            Action::OpenMagic => self.open(1, sora),
            Action::OpenItems => self.open(2, sora),
            Action::OpenDrive => self.open(3, sora),
            Action::Magic(slot) => {
                self.choose(Choice::Magic { slot });
                self.close_pages();
            }
            Action::Item(id) => {
                self.choose(Choice::Item { id });
                self.close_pages();
            }
            Action::Drive(form) => {
                self.choose(Choice::Drive { form });
                self.close_pages();
            }
            Action::None => {}
        }
    }

    fn choose(&mut self, c: Choice) {
        self.choices.push(c);
        if self.choices.len() > 64 {
            self.choices.remove(0);
        }
        self.fresh.push(c);
    }

    fn close_pages(&mut self) {
        self.menu.pages.truncate(1);
        self.actions.truncate(1);
    }

    fn set(&mut self, node: usize, group: Option<usize>) {
        let (Some(f), Some(n)) = (self.menu_file, self.nodes.get_mut(node)) else { return };
        match group {
            None => n.visible = false,
            Some(g) => {
                n.visible = true;
                if n.player.group != Some(g) {
                    n.player.set_group(&self.files[f].seq, g);
                    n.label_time = 0.0;
                }
            }
        }
    }

    fn label(&self, message: u32, color: [u8; 4]) -> Option<Label> {
        let t = self.text.as_ref()?;
        let tokens = t.lookup(message)?;
        Some(Label { tokens, style: Style { world: t.world, font: hm::TEXT_FONT, size: Some(hm::TEXT_SIZE), color, ..Style::default() }, offset: hm::TEXT_OFFSET })
    }

    /// One 60 Hz tick: the menu nodes from the menu (kh2-rs `hud_menu::update`), then the players.
    pub fn tick(&mut self, sora: &crate::sora::Sora) {
        // The reaction capsule mirrors the offered command (kh2-rs `mirror_reaction`).
        self.menu.reaction = self.reaction_cmd.and_then(|id| self.cmds.iter().find(|c| c.id == id)).map(|c| Reaction { message: c.message_id as u32, alt: c.flags & 8 != 0, kind7: false });
        if self.menu_file.is_some()
            && let Some(tb) = self.tables.take()
        {
            self.layout(&tb);
            self.tables = Some(tb);
        }
        for n in &mut self.nodes {
            n.player.tick(1.0);
            if n.label.is_some() && n.visible {
                n.label_time += 1.0;
            }
        }
    }

    fn layout(&mut self, tb: &Tables) {
        let show = self.menu.visible;
        let last_count = self.menu.pages.last().map_or(0, |p| p.entries.len());
        for p in 0..hm::PAGES {
            let Some(page) = self.menu.pages.get(p).filter(|_| show).cloned() else {
                for id in std::iter::once(self.header[p]).chain(self.rows[p]).chain(self.selected[p]) {
                    self.nodes[id].visible = false;
                    self.nodes[id].label = None;
                }
                continue;
            };
            let k = (page.kind as usize).min(hm::KINDS - 1);
            let row = tb.kind[k];
            let n = page.entries.len().min(hm::ROWS);
            let x = hm::page_x(p);
            // Header: the commands banner opens then idles; the other menus keep their own group.
            let h = self.header[p];
            let animated = matches!(page.kind, 0 | 6);
            let want = if animated { tb.idle[p] as usize } else { row[p] as usize };
            if animated {
                let first = tb.open[p] as usize;
                let cur = self.nodes[h].player.group;
                if cur != Some(first) && cur != Some(want) {
                    self.set(h, Some(first));
                } else if cur == Some(first) && self.nodes[h].player.finished() {
                    self.set(h, Some(want));
                }
            } else {
                self.set(h, Some(want));
            }
            self.nodes[h].pos = [x, hm::header_y(n)];
            self.nodes[h].visible = true;
            for r in 0..hm::ROWS {
                if r >= n {
                    self.nodes[self.rows[p][r]].visible = false;
                    self.nodes[self.rows[p][r]].label = None;
                    continue;
                }
                let e = page.entries[r].clone();
                let sel = r == page.cursor;
                let bar = if sel { self.selected[p][0] } else { self.rows[p][r] };
                if sel {
                    self.nodes[self.rows[p][r]].visible = false;
                    self.nodes[self.rows[p][r]].label = None;
                }
                let y = hm::row_y(n, r);
                let g = hm::bar_group(&row, p, r, n, sel);
                let bx = if sel { x + 8.0 } else { x };
                self.set(bar, Some(g));
                self.nodes[bar].pos = [bx, y];
                let color = if sel { tb.selected } else { tb.normal }[(e.style as usize).min(8)];
                self.nodes[bar].label = self.label(e.message, color);
                if sel {
                    let first = hm::shows_icons(p);
                    let [_, plate, icon, arrow] = self.selected[p];
                    self.set(plate, first.then_some(tb.plate[p] as usize));
                    self.set(icon, hm::icon_group(e.icon).filter(|_| first));
                    self.set(arrow, first.then_some(hm::ARROW[(page.kind != 0) as usize]));
                    for id in [plate, icon, arrow] {
                        self.nodes[id].pos = [bx, y];
                    }
                }
            }
            if page.cursor >= n {
                for id in self.selected[p] {
                    self.nodes[id].visible = false;
                    self.nodes[id].label = None;
                }
            }
        }
        // The reaction capsule above the list.
        let c = self.capsule;
        let group = match (&self.menu.reaction, show) {
            (Some(r), true) => Some(if r.kind7 { tb.reaction[1] } else if r.alt { tb.reaction[2] } else { tb.reaction[0] } as usize),
            (None, true) => Some(tb.reaction[3] as usize),
            _ => None,
        };
        self.set(c, group);
        self.nodes[c].pos = [36.0, hm::capsule_y(last_count)];
        let label = self.menu.reaction.clone().filter(|_| show).and_then(|r| self.label(r.message, [0xff, 0xff, 0xff, 0x80]));
        self.nodes[c].label = label;
    }

    /// The draw list in draw order (kh2-rs `HudScene::quads`): each visible node's sequence quads,
    /// then its label's glyphs.
    pub fn draw_list(&self) -> Vec<Sprite> {
        let mut out = Vec::new();
        for n in self.nodes.iter().filter(|n| n.visible) {
            if let (Some(g), Some(f)) = (n.player.group, self.files.get(n.file)) {
                let ext = Ext { time: n.player.uv_time, ..Ext::default() };
                for q in f.seq.quads(g, n.player.frame(), n.pos, &ext) {
                    out.push(Sprite { texture: f.texture, pos: q.pos, uv: q.uv, color: q.color, blend: q.blend, nearest: q.nearest, text: false });
                }
            }
            if let (Some(label), Some(t)) = (&n.label, &self.text) {
                for (tex, q) in t.quads(label, n.pos, n.label_time) {
                    out.push(Sprite { texture: tex, pos: q.pos, uv: q.uv, color: q.color, blend: q.blend, nearest: q.nearest, text: true });
                }
            }
        }
        out
    }

    /// Software rasteriser of the draw list onto an RGBA image of `w` x `h` (the 512 x 448 screen
    /// scaled), for tests and the viewer: each quad as two triangles, nearest texel, colour times
    /// texel / 128, alpha blending (additive for blend 1).
    pub fn rasterize(&self, list: &[Sprite], w: usize, h: usize, out: &mut [u8]) {
        let (sx, sy) = (w as f32 / SCREEN[0], h as f32 / SCREEN[1]);
        for s in list {
            let Some(t) = self.textures.get(s.texture as usize) else { continue };
            let p = s.pos.map(|q| [q[0] * sx, q[1] * sy]);
            for tri in [[0usize, 1, 2], [1, 3, 2]] {
                let [a, b, c] = tri.map(|k| p[k]);
                let (x0, x1) = (a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize, a[0].max(b[0]).max(c[0]).ceil().min(w as f32) as usize);
                let (y0, y1) = (a[1].min(b[1]).min(c[1]).floor().max(0.0) as usize, a[1].max(b[1]).max(c[1]).ceil().min(h as f32) as usize);
                let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                if area.abs() < 1e-6 {
                    continue;
                }
                for y in y0..y1 {
                    for x in x0..x1 {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
                        let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let [ua, ub, uc] = tri.map(|k| s.uv[k]);
                        let u = w0 * ua[0] + w1 * ub[0] + w2 * uc[0];
                        let v = w0 * ua[1] + w1 * ub[1] + w2 * uc[1];
                        let tx = (u.floor() as i64).rem_euclid(t.width.max(1) as i64) as usize;
                        let ty = (v.floor() as i64).rem_euclid(t.height.max(1) as i64) as usize;
                        let ti = (ty * t.width as usize + tx) * 4;
                        let col = s.color[tri[0]];
                        let texel = &t.rgba[ti..ti + 4];
                        let mut c4 = [0f32; 4];
                        for k in 0..3 {
                            c4[k] = (texel[k] as f32 * col[k] as f32 / 128.0).min(255.0);
                        }
                        c4[3] = (texel[3] as f32 * col[3] as f32 / 128.0).min(255.0) / 255.0;
                        let o = (y * w + x) * 4;
                        for k in 0..3 {
                            let d = out[o + k] as f32;
                            out[o + k] = if s.blend == 1 { (d + c4[k] * c4[3]).min(255.0) } else { d + (c4[k] - d) * c4[3] } as u8;
                        }
                        out[o + 3] = 255;
                    }
                }
            }
        }
    }
}
