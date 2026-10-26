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

use kh2_enemy_host::hud_gauge::{self as hg, dg, g, DriveGauge, DriveMode, DriveTables, PanelEvent, PanelState};
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

/// One HUD object (kh2-rs `hud_gfx::Node`): a sequence player at a position or under a parent.
struct Node {
    file: usize,
    player: Player,
    parent: Option<usize>,
    pos: [f32; 2],
    off: [f32; 2],
    ext: Ext,
    visible: bool,
    /// Clip to the wedge from `sweep_start` clockwise by this many radians (a ring gauge).
    sweep: Option<f32>,
    sweep_start: f32,
    /// Show only this fraction of the width from the left (a gauge fill).
    reveal: Option<f32>,
    /// The follow-up group once a limited group has finished (0x138aa8).
    next: Option<usize>,
    label: Option<Label>,
    label_time: f32,
    anchor: [f32; 2],
    base: [f32; 2],
    origin: [f32; 2],
}

impl Node {
    fn new(file: usize) -> Node {
        Node { file, player: Player::default(), parent: None, pos: [0.0; 2], off: [0.0; 2], ext: Ext::default(), visible: false, sweep: None, sweep_start: 0.0, reveal: None, next: None, label: None, label_time: 0.0, anchor: [0.0; 2], base: [0.0; 2], origin: [0.0; 2] }
    }
}

/// kh2-rs `hud_gfx::Y_BIAS`: HUD y 0 is 16 lines below the top of the picture.
pub const Y_BIAS: f32 = 16.0;

#[derive(Clone, Copy)]
struct Vert {
    pos: [f32; 2],
    uv: [f32; 2],
    col: [f32; 4],
}

fn lerp_vert(a: Vert, b: Vert, t: f32) -> Vert {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Vert { pos: [l(a.pos[0], b.pos[0]), l(a.pos[1], b.pos[1])], uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])], col: [l(a.col[0], b.col[0]), l(a.col[1], b.col[1]), l(a.col[2], b.col[2]), l(a.col[3], b.col[3])] }
}

/// kh2-rs `hud_gfx::clip`: the part of a convex polygon where `dot(p - c, n) >= 0`.
fn clip(poly: &[Vert], c: [f32; 2], n: [f32; 2]) -> Vec<Vert> {
    let d = |v: &Vert| (v.pos[0] - c[0]) * n[0] + (v.pos[1] - c[1]) * n[1];
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (da, db) = (d(&a), d(&b));
        if da >= 0.0 {
            out.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(lerp_vert(a, b, da / (da - db)));
        }
    }
    out
}

/// kh2-rs `hud_gfx::wedge`: the wedge of a ring gauge from `start` (radians clockwise from west) by `sweep`.
fn wedge(poly: &[Vert], c: [f32; 2], start: f32, sweep: f32) -> Vec<Vec<Vert>> {
    use std::f32::consts::PI;
    if sweep <= 0.0 {
        return Vec::new();
    }
    let edge = |a: f32| [-a.sin(), a.cos()];
    if sweep >= 2.0 * PI {
        return vec![poly.to_vec()];
    }
    let first = clip(poly, c, edge(start + PI));
    if sweep <= PI {
        return vec![clip(&first, c, edge(start + sweep))];
    }
    let second = clip(&clip(poly, c, edge(start)), c, edge(start + sweep));
    vec![first, second]
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

/// Sora's HP / MP panel and drive gauge (kh2-rs `hud_gauge::SoraPanel`).
#[derive(Default)]
struct Panel {
    circle: usize,
    face: Option<usize>,
    ring: [usize; 3],
    bar: [usize; 3],
    cap: usize,
    mp: [usize; 4],
    drive: [usize; 5],
    drive_mode: Option<DriveMode>,
    drive_points: Option<u8>,
    recharging: bool,
    state: Option<PanelState>,
    last_hp: i32,
    lag_hp: i32,
    alarm: bool,
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
    /// The field file's first pair index (`fiel`; `gaug` follows, kh2-rs `hud_gauge::GAUG`) and Sora's face.
    field: Option<usize>,
    face_file: Option<usize>,
    drive_tables: DriveTables,
    panel: Option<Panel>,
    /// Sounds the panel asks for (the danger alarm: system sound 9).
    pub sounds: Vec<u32>,
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
            field: None,
            face_file: None,
            drive_tables: DriveTables::from_exe(&exe),
            panel: None,
            sounds: Vec::new(),
        };
        let pair = |b: &[u8]| -> Option<(kh2_image::imgd::Image, Seqd)> {
            let e = kh2_bar::parse(b).ok()?;
            e.windows(2).find_map(|w| Some((kh2_image::imgd::decode(w[0].data)?, kh2_image::seqd::parse(w[1].data)?)))
        };
        // The world's field file (pairs fiel, gaug, mapj, miss) and Sora's face (the last pair of his APDX), as
        // kh2-rs `hud::load_files`.
        let pairs = |b: &[u8]| -> Vec<(kh2_image::imgd::Image, Seqd)> {
            let Ok(e) = kh2_bar::parse(b) else { return Vec::new() };
            e.windows(2).filter_map(|w| Some((kh2_image::imgd::decode(w[0].data)?, kh2_image::seqd::parse(w[1].data)?))).collect()
        };
        let mut field = disc.read(&format!("field2d/us/{world}0field.2dd")).map(|b| pairs(&b)).unwrap_or_default();
        if field.is_empty() {
            field = disc.read("field2d/us/zz0field.2dd").map(|b| pairs(&b)).unwrap_or_default();
        }
        if !field.is_empty() {
            hud.field = Some(hud.files.len());
            for (img, seq) in field {
                hud.textures.push(Texture { width: img.width, height: img.height, rgba: img.rgba });
                hud.files.push(SeqFile { texture: (hud.textures.len() - 1) as u32, seq });
            }
            if let Some((img, seq)) = disc.read("obj/P_EX100.apdx").map(|b| pairs(&b)).unwrap_or_default().pop() {
                hud.textures.push(Texture { width: img.width, height: img.height, rgba: img.rgba });
                hud.files.push(SeqFile { texture: (hud.textures.len() - 1) as u32, seq });
                hud.face_file = Some(hud.files.len() - 1);
            }
        }
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

    fn add(&mut self, file: usize, parent: Option<usize>, group: Option<usize>) -> usize {
        let mut n = Node::new(file);
        n.parent = parent;
        if let (Some(g), Some(f)) = (group, self.files.get(file)) {
            n.player.set_group(&f.seq, g);
            n.visible = true;
        }
        self.nodes.push(n);
        self.nodes.len() - 1
    }

    /// Starts `group` on a node (restarting it), as kh2-rs `HudScene::set_group`.
    fn group(&mut self, node: usize, group: usize) {
        let file = self.nodes[node].file;
        if let Some(f) = self.files.get(file) {
            self.nodes[node].player.set_group(&f.seq, group);
        }
    }

    /// kh2-rs `HudScene::set_group_then`.
    fn group_then(&mut self, node: usize, group: usize, next: usize) {
        self.group(node, group);
        self.nodes[node].next = Some(next);
    }

    fn set(&mut self, node: usize, group: Option<usize>) {
        let Some(n) = self.nodes.get_mut(node) else { return };
        let f = n.file;
        if f >= self.files.len() {
            return;
        }
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
    pub fn tick(&mut self, _sora: &crate::sora::Sora) {
        // The reaction capsule mirrors the offered command (kh2-rs `mirror_reaction`).
        self.menu.reaction = self.reaction_cmd.and_then(|id| self.cmds.iter().find(|c| c.id == id)).map(|c| Reaction { message: c.message_id as u32, alt: c.flags & 8 != 0, kind7: false });
        if self.menu_file.is_some()
            && let Some(tb) = self.tables.take()
        {
            self.layout(&tb);
            self.tables = Some(tb);
        }
        self.gauge_tick(_sora);
        let files = &self.files;
        for n in &mut self.nodes {
            n.player.tick(1.0);
            // 0x138aa8: a finished group hands over to its follow-up, stepped once.
            if n.player.finished()
                && let (Some(g), Some(f)) = (n.next.take(), files.get(n.file))
            {
                n.player.set_group(&f.seq, g);
                n.player.tick(1.0);
            }
            if n.label.is_some() && n.visible {
                n.label_time += 1.0;
            }
        }
        self.resolve();
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

    /// kh2-rs `hud_gauge::build`: the panel's objects in the game's creation order.
    fn gauge_build(&mut self, hp: i32) {
        let Some(field) = self.field else { return };
        let gaug = field + hg::GAUG;
        let mut p = Panel::default();
        p.circle = self.add(gaug, None, Some(g::CIRCLE[0]));
        p.ring = [self.add(gaug, Some(p.circle), Some(g::RING_BACK[0])), self.add(gaug, Some(p.circle), Some(g::RING_DEFAULT)), self.add(gaug, Some(p.circle), Some(g::RING_GREEN))];
        p.bar = [self.add(gaug, Some(p.circle), Some(g::BAR_BACK[0])), self.add(gaug, Some(p.circle), Some(g::BAR_TRAIL)), self.add(gaug, Some(p.circle), Some(g::BAR_GREEN))];
        p.cap = self.add(gaug, Some(p.circle), Some(g::CAP));
        p.face = self.face_file.map(|f| self.add(f, Some(p.circle), Some(g::FACE[0])));
        let label = self.add(gaug, Some(p.circle), Some(g::MP_LABEL));
        p.mp = [label, self.add(gaug, Some(label), Some(g::MP_BACK)), self.add(gaug, Some(label), Some(g::MP_FILL)), self.add(gaug, Some(label), Some(g::MP_CAP))];
        for i in p.bar {
            self.nodes[i].ext.scale[1] = 0.0;
        }
        let bar = self.add(gaug, Some(p.circle), None);
        p.drive = [bar, self.add(gaug, Some(bar), None), self.add(gaug, Some(bar), None), self.add(gaug, Some(bar), None), self.add(gaug, Some(bar), None)];
        self.group(p.drive[3], dg::MAX);
        let at = self.files.get(gaug).and_then(|f| f.seq.groups.get(dg::intro(DriveMode::Normal))).map_or([0.0, 0.0], |gr| [gr.x as f32, gr.y as f32]);
        self.nodes[p.drive[2]].off = at;
        p.last_hp = hp;
        p.lag_hp = hp;
        self.panel = Some(p);
    }

    /// kh2-rs `hud_gauge::update` / `panel_frame` / `update_drive` with Sora's values.
    fn gauge_tick(&mut self, sora: &crate::sora::Sora) {
        let (hp, max) = (sora.hp.max(0), sora.max_hp.max(1));
        let (mp, mp_max) = (sora.mp.clamp(0, sora.max_mp.max(0)), sora.max_mp.max(1));
        if self.field.is_none() {
            return;
        }
        if self.panel.is_none() {
            self.gauge_build(hp);
        }
        let Some(mut p) = self.panel.take() else { return };
        let ko = matches!(sora.state, crate::sora::State::Down);
        let state = PanelState::of(hp, max, ko);
        let s = state as usize;
        let last = p.state.unwrap_or(PanelState::Normal);
        match PanelEvent::of(p.last_hp, hp, last, state) {
            PanelEvent::Hit => {
                self.group_then(p.circle, g::CIRCLE_HIT, g::CIRCLE[s]);
                self.group(p.ring[0], g::RING_BACK[s]);
                self.group(p.bar[0], g::BAR_BACK[s]);
                if let Some(f) = p.face {
                    self.group_then(f, g::FACE_HIT, g::FACE[s]);
                }
            }
            PanelEvent::Heal | PanelEvent::Switch => {
                self.group(p.circle, g::CIRCLE[s]);
                self.group(p.ring[0], g::RING_BACK[s]);
                self.group(p.bar[0], g::BAR_BACK[s]);
                if let Some(f) = p.face {
                    self.group(f, g::FACE[s]);
                }
            }
            PanelEvent::None => {}
        }
        p.state = Some(state);
        if hp != p.last_hp {
            p.lag_hp = if p.last_hp < hp { hp } else { p.last_hp };
            p.last_hp = hp;
            self.group(p.ring[1], g::RING_TRAIL);
            self.group(p.bar[1], g::BAR_TRAIL);
        }
        if hg::alarm_step(&mut p.alarm, state) == Some(true) {
            self.sounds.push(hg::DANGER_ALARM_SE);
        }
        for (layer, value) in [(0, max), (1, p.lag_hp), (2, hp)] {
            let (ring, bar) = (p.ring[layer], p.bar[layer]);
            match hg::fill(value) {
                hg::Fill::Ring(sweep) => {
                    self.nodes[ring].sweep = Some(sweep);
                    self.nodes[bar].visible = false;
                }
                hg::Fill::Bar(len) => {
                    self.nodes[ring].sweep = None;
                    let n = &mut self.nodes[bar];
                    n.visible = true;
                    n.ext.scale[1] = len / hg::BAR_SPRITE;
                }
            }
        }
        let cap = &mut self.nodes[p.cap];
        match hg::fill(max) {
            hg::Fill::Ring(sweep) => {
                cap.ext.rot = Some([0.0, 0.0, -sweep]);
                cap.off = [0.0, 0.0];
            }
            hg::Fill::Bar(len) => {
                cap.ext.rot = Some([0.0, 0.0, std::f32::consts::FRAC_PI_2]);
                cap.off = [-len, 0.0];
            }
        }
        // MP (the recharge look while the MP charge runs, 0x157628).
        let charging = sora.charge.active();
        if charging != p.recharging {
            p.recharging = charging;
            let set = if charging { hg::MP_RECHARGE } else { hg::MP_NORMAL };
            for (node, gr) in p.mp.into_iter().zip(set) {
                self.group(node, gr);
            }
        }
        if charging {
            let ratio = if sora.charge.total > 0.0 { 1.0 - sora.charge.left / sora.charge.total } else { 0.0 };
            self.nodes[p.mp[1]].ext.scale[1] = 1.0;
            self.nodes[p.mp[2]].ext.scale[1] = ratio;
            self.nodes[p.mp[3]].off = [-hg::BAR_SPRITE, 0.0];
        } else {
            self.nodes[p.mp[1]].ext.scale[1] = mp_max as f32 / hg::BAR_SPRITE;
            self.nodes[p.mp[2]].ext.scale[1] = mp as f32 / hg::BAR_SPRITE;
            self.nodes[p.mp[3]].off = [-(mp_max as f32), 0.0];
        }
        // The drive gauge (kh2-rs `gauge_of` + `update_drive`).
        let d = &sora.drive;
        let gauge = if d.in_form {
            let (digit, fill, seg) = kh2_game_core::drive_rules::segment_display(d.timer.left, d.timer.total, d.points.max.max(1));
            DriveGauge { enabled: true, mode: DriveMode::FormA, points: digit.min(9) as u8, max: d.points.max, fraction: 0, time_fill: Some(if seg > 0 { fill as f32 / seg as f32 } else { 0.0 }) }
        } else {
            DriveGauge { enabled: true, mode: DriveMode::Normal, points: d.points.whole, max: d.points.max, fraction: d.points.fraction, time_fill: None }
        };
        self.drive_frame(&mut p, &gauge);
        self.panel = Some(p);
    }

    fn drive_frame(&mut self, p: &mut Panel, d: &DriveGauge) {
        let [bar, fill, digit, max, burst] = p.drive;
        let Some(table) = self.drive_tables.0 else { return };
        if p.drive_mode != Some(d.mode) {
            p.drive_mode = Some(d.mode);
            self.nodes[bar].visible = true;
            self.group(bar, dg::idle(d.mode));
            self.group(fill, dg::fill(d.mode));
            p.drive_points = None;
        }
        let set = match d.mode {
            DriveMode::FormA => 1,
            DriveMode::FormB => 2,
            _ => 0,
        };
        let pts = d.points.min(9);
        if p.drive_points != Some(pts) {
            if p.drive_points.is_some_and(|old| pts > old) {
                self.group(burst, dg::BURST);
                self.nodes[burst].visible = true;
            }
            p.drive_points = Some(pts);
            self.group(digit, table[set][pts as usize] as usize);
        }
        if self.nodes[burst].player.finished() {
            self.nodes[burst].visible = false;
        }
        let full = d.points >= d.max;
        let reveal = match d.time_fill {
            Some(f) if d.mode != DriveMode::Normal => f.clamp(0.0, 1.0),
            _ if full => 1.0,
            _ => d.fraction.min(99) as f32 / 100.0,
        };
        self.nodes[fill].reveal = Some(reveal);
        self.nodes[max].visible = full && d.mode == DriveMode::Normal;
        self.nodes[bar].visible = true;
        for n in [fill, digit] {
            self.nodes[n].visible = d.mode != DriveMode::Locked;
        }
    }

    /// kh2-rs `HudScene::resolve`: positions, parents first (a parent always has a lower index).
    fn resolve(&mut self) {
        for i in 0..self.nodes.len() {
            let base = match self.nodes[i].parent {
                Some(p) => {
                    let pn = &self.nodes[p];
                    let anchor = match (pn.player.group, self.files.get(pn.file)) {
                        (Some(g), Some(f)) => f.seq.anchor(g, pn.player.frame(), &pn.ext).map(|a| [a[0] as f32, a[1] as f32]),
                        _ => None,
                    };
                    let anchor = anchor.unwrap_or(pn.anchor);
                    let (pb, po) = (pn.base, pn.off);
                    self.nodes[p].anchor = anchor;
                    [pb[0] + anchor[0] + po[0], pb[1] + anchor[1] + po[1]]
                }
                None => self.nodes[i].pos,
            };
            let n = &mut self.nodes[i];
            n.base = base;
            n.origin = [base[0] + n.off[0], base[1] + n.off[1]];
        }
    }

    /// The draw list in draw order (kh2-rs `HudScene::quads` and `draw`): each visible node's sequence
    /// quads (clipped to its ring wedge and its reveal), then its label's glyphs; screen y includes
    /// `Y_BIAS`. A clipped quad comes out as triangles (`pos[3] == pos[2]`).
    pub fn draw_list(&self) -> Vec<Sprite> {
        let mut out = Vec::new();
        let push = |out: &mut Vec<Sprite>, tex: u32, poly: &[Vert], blend: u32, nearest: bool, text: bool| {
            if poly.len() < 3 {
                return;
            }
            let col = |v: &Vert| [v.col[0], v.col[1], v.col[2], v.col[3]].map(|c| c.clamp(0.0, 255.0) as u8);
            let pos = |v: &Vert| [v.pos[0], v.pos[1] + Y_BIAS];
            if poly.len() == 4 {
                // TL, TR, BR, BL back to the sprite's TL, TR, BL, BR.
                let o = [0usize, 1, 3, 2];
                out.push(Sprite { texture: tex, pos: o.map(|k| pos(&poly[k])), uv: o.map(|k| poly[k].uv), color: o.map(|k| col(&poly[k])), blend, nearest, text });
                return;
            }
            for k in 1..poly.len() - 1 {
                let tri = [poly[0], poly[k], poly[k + 1], poly[k + 1]];
                out.push(Sprite { texture: tex, pos: tri.map(|v| pos(&v)), uv: tri.map(|v| v.uv), color: tri.map(|v| col(&v)), blend, nearest, text });
            }
        };
        for n in self.nodes.iter().filter(|n| n.visible) {
            if let (Some(g), Some(f)) = (n.player.group, self.files.get(n.file)) {
                let ext = Ext { time: n.player.uv_time, ..n.ext };
                let qs = f.seq.quads(g, n.player.frame(), n.origin, &ext);
                let max_x = n.reveal.map(|r| {
                    let (lo, hi) = qs.iter().flat_map(|q| q.pos.iter().map(|p| p[0])).fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
                    lo + (hi - lo) * r.clamp(0.0, 1.0)
                });
                for q in qs {
                    let poly: Vec<Vert> = [0usize, 1, 3, 2].iter().map(|&k| Vert { pos: q.pos[k], uv: q.uv[k], col: q.color[k].map(f32::from) }).collect();
                    let mut pieces = match n.sweep {
                        Some(s) => wedge(&poly, n.origin, n.sweep_start, s),
                        None => vec![poly],
                    };
                    if let Some(mx) = max_x {
                        pieces = pieces.iter().map(|p| clip(p, [mx, 0.0], [-1.0, 0.0])).collect();
                    }
                    for p in &pieces {
                        push(&mut out, f.texture, p, q.blend, q.nearest, false);
                    }
                }
            }
            if let (Some(label), Some(t)) = (&n.label, &self.text) {
                for (tex, q) in t.quads(label, n.origin, n.label_time) {
                    let poly: Vec<Vert> = [0usize, 1, 3, 2].iter().map(|&k| Vert { pos: q.pos[k], uv: q.uv[k], col: q.color[k].map(f32::from) }).collect();
                    push(&mut out, tex, &poly, q.blend, q.nearest, true);
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
