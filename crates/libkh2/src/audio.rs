//! An audio engine for hosts (round 10, like libsm64's `sm64_audio_tick`): the library's own sound
//! cues (`crate::cue`) played through kh2-rs's software sound driver (`kh2_audio::ssl::Driver`: the
//! game's SE sequencer on a software SPU2, the tables read from the disc's `LIBSSL.IRX`), rendered
//! to interleaved 16-bit stereo PCM at 48 kHz.
//!
//! What sounds: trigger-8 motion sounds (bank * 1000 + index: the owner's own bank in
//! `obj/<model>.apdx` when the request names bank 500 or the bank the object's block carries, else
//! the global `se/seNNN.seb` with its wave bank `se/waveNNNN.wd`, kh2-rs `audio::load_bank`) and
//! the numbered sounds (the mission's), the hit sounds (kh2-rs `PlaySe::hit`: the attack row's
//! `EnemyReaction` class, light 0 / 1 or heavy 2 / 3 of the attacker's own bank, alternating per hit)
//! and an area's music (`play_bgm`: `bgm/musicNNN.bgm` with its wave bank, kh2-rs `load_sequence`).
//! A placed sound takes its volume and pan from the listener the host sets (kh2-rs
//! `sound::placement` -> `kh2_audio::positional::volume_pan`: the source in the listener's frame,
//! game units); without a listener every sound plays centred at full level. Voices and footsteps
//! (the voice table, the area's sound info) are not played.

use std::collections::HashMap;
use std::sync::Arc;

use kh2_audio::ssl::{Driver, SeBlock, Tables};
use kh2_audio::Wd;

use crate::cue::{Cue, Kind};
use crate::data::Disc;

/// The output rate (`kh2_audio::RATE`).
pub const RATE: u32 = kh2_audio::RATE;
/// kh2-rs `audio::OWN_BANK`: a request's bank number meaning "the object's own bank".
const OWN_BANK: u16 = 500;
/// kh2fm-rs `VOICE_BAR_ENTRY`: the entry type of an object's own voice bank in its `.apdx`.
const VOICE_BAR_ENTRY: u16 = 0x22;

struct Bank {
    block: SeBlock,
    wd: Option<Arc<Wd>>,
}

pub struct Audio {
    driver: Driver,
    banks: HashMap<String, Option<Arc<Bank>>>,
    wds: HashMap<u16, Option<Arc<Wd>>>,
    /// Cues waiting for the next render.
    pub pending: Vec<Cue>,
    /// Requests that found no sound (bank, index), for the host's log.
    pub missing: Vec<(u16, u16)>,
    pub played: u32,
    /// The listener: position (cm) and its right, up and forward axes.
    pub listener: Option<[glam::Vec3; 4]>,
    /// The hit counter the hit sounds alternate on.
    pub hits: u8,
    /// Voices (kh2fm-rs `VoiceTable` / `VoiceInfo`): the `vtbl` rows, the speaker names and the
    /// voice language folder from the executable, the shared generator of the pick.
    vtbl: Vec<kh2_tables::tables::Vtbl>,
    names: kh2_audio::voice::SpeakerNames,
    lang: String,
    pub rng: u32,
    vsbs: HashMap<String, Option<Arc<kh2_audio::vsb::Vsb>>>,
    /// One voice slot per speaker model (the priority rule of FUN_001ae798).
    slots: HashMap<String, kh2_audio::voice::VoiceSlot>,
    /// The area (`kh2_audio_set_area`): its name, its `arif` voice bank and its `12soundinfo` record.
    pub area: Option<String>,
    voice_bank: u16,
    soundinfo: Option<kh2_audio::footstep::SoundInfoRecord>,
    /// `objentry` NeoStatus by model (the footstep bank pick); the footstep counters by owner.
    neo: HashMap<String, u16>,
    steps: HashMap<String, u8>,
    /// Voice lines played (for the host's log and the tests).
    pub voices: u32,
}

/// The `objentry` row's `NeoStatus` of `model` (kh2fm-rs `neo_status`).
fn neo_status(disc: &mut Disc, model: &str) -> Option<u16> {
    let rows = kh2_tables::load::<kh2_tables::tables::Objentry>(&mut disc.0, &kh2_tables::tables::OBJENTRY).ok()?.rows;
    rows.iter().find(|r| r.model_name == model).map(|r| r.neo_status as u16)
}

/// The `arif` record's voice bank of an area such as `tt04` (kh2fm-rs `arif_record`: world by the
/// game's order, record 0x40 bytes, the voice bank at +0x30).
fn arif_voice_bank(disc: &mut Disc, area: &str) -> Option<u16> {
    let sys = disc.read("03system.bin").ok()?;
    let e = kh2_bar::parse(&sys).ok()?;
    let arif = e.iter().find(|e| e.tag == "arif")?.data;
    let (w, a) = area.split_at_checked(2)?;
    let world = kh2_image::text::WORLDS.iter().position(|x| *x == w)?;
    let index: usize = a.parse().ok()?;
    let u16_at = |o: usize| arif.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]));
    let count = u16_at(8 + 4 * world)? as usize;
    let off = u16_at(8 + 4 * world + 2)? as usize;
    (index < count).then_some(())?;
    u16_at(off + 0x40 * index + 0x30)
}

impl Audio {
    pub fn new(disc: &mut Disc) -> Result<Audio, String> {
        let irx = disc.0.iso().read("LIBSSL.IRX").map_err(|e| format!("LIBSSL.IRX: {e}"))?;
        let tables = Tables::from_irx(&irx).ok_or("LIBSSL.IRX: no driver tables")?;
        let mut driver = Driver::new(Arc::new(tables));
        // The game's output stage is at full level (the SPU2 main volume 0x3fff, the master factor of
        // the 0x10b0b0 chain); 1.x used kh2-extras' 0.6.
        driver.mixer.master = 1.0;
        let vtbl = kh2_tables::load::<kh2_tables::tables::Vtbl>(&mut disc.0, &kh2_tables::tables::VTBL).map(|t| t.rows).unwrap_or_default();
        let exe = disc.exe();
        let names = exe.as_deref().and_then(kh2_audio::voice::SpeakerNames::from_exe).unwrap_or_default();
        let lang = exe.as_deref().and_then(kh2_audio::voice::language).unwrap_or_else(|| "us".into());
        Ok(Audio {
            driver,
            banks: HashMap::new(),
            wds: HashMap::new(),
            pending: Vec::new(),
            missing: Vec::new(),
            played: 0,
            listener: None,
            hits: 0,
            vtbl,
            names,
            lang,
            rng: 1,
            vsbs: HashMap::new(),
            slots: HashMap::new(),
            area: None,
            voice_bank: 0,
            soundinfo: None,
            neo: HashMap::new(),
            steps: HashMap::new(),
            voices: 0,
        })
    }

    fn bank(&mut self, disc: &mut Disc, name: &str, global: bool) -> Option<Arc<Bank>> {
        self.banks
            .entry(name.to_string())
            .or_insert_with(|| {
                let raw = disc.read(name).ok()?;
                if global {
                    return SeBlock::parse(&raw).map(|block| Arc::new(Bank { block, wd: None }));
                }
                let bar = kh2_bar::parse(&raw).ok()?;
                let block = bar.iter().find(|e| e.tag.starts_with("se")).and_then(|e| SeBlock::parse(e.data))?;
                let wd = bar.iter().find(|e| e.tag == "wave").and_then(|e| Wd::parse(e.data)).map(Arc::new);
                Some(Arc::new(Bank { block, wd }))
            })
            .clone()
    }

    fn wd(&mut self, disc: &mut Disc, id: u16) -> Option<Arc<Wd>> {
        self.wds.entry(id).or_insert_with(|| disc.read(&format!("se/wave{id:04}.wd")).ok().and_then(|b| Wd::parse(&b)).map(Arc::new)).clone()
    }

    /// kh2-rs `sound::placement`: volume and pan of a sound at `pos` for the listener.
    pub fn placement(&self, pos: Option<glam::Vec3>) -> (i32, i32) {
        use kh2_audio::positional::{volume_pan, CENTRE_PAN, FULL_VOLUME};
        let (Some(p), Some([at, right, up, fwd])) = (pos, self.listener) else { return (FULL_VOLUME, CENTRE_PAN) };
        let d = p - at;
        volume_pan([d.dot(right), d.dot(up), d.dot(fwd)])
    }

    /// Starts the area music `music<number>` (kh2-rs `play_bgm`: the field slot, full level); false
    /// when the file or its wave bank is missing.
    pub fn play_bgm(&mut self, disc: &mut Disc, number: u32) -> bool {
        let Ok(raw) = disc.read(&format!("bgm/music{number:03}.bgm")) else { return false };
        let Some(bgm) = kh2_audio::Bgm::parse(&raw) else { return false };
        let Some(wd) = disc.read(&format!("bgm/wave{:04}.wd", bgm.wd)).ok().and_then(|b| Wd::parse(&b)) else { return false };
        let Some(seq) = kh2_audio::ssl::Sequence::bgm(&raw, Arc::new(wd), 0) else { return false };
        self.driver.bgm_stop(1, false);
        self.driver.play_bgm(seq);
        true
    }

    pub fn stop_bgm(&mut self) {
        self.driver.bgm_stop(1, false);
    }

    /// The area the sounds belong to (`"tt04"`): its voice bank (`arif`) and footstep record
    /// (`12soundinfo.bar`, kh2fm-rs `area_soundinfo`).
    pub fn set_area(&mut self, disc: &mut Disc, area: &str) {
        self.voice_bank = arif_voice_bank(disc, area).unwrap_or(0);
        self.soundinfo = (|| {
            let (world, number) = area.split_at_checked(2)?;
            let raw = disc.read("12soundinfo.bar").ok()?;
            let e = kh2_bar::parse(&raw).ok()?;
            let entry = e.iter().find(|e| e.tag == world)?;
            kh2_audio::footstep::WorldSoundInfo::parse(entry.data)?.area(number.parse().ok()?).copied()
        })();
        self.area = Some(area.to_string());
    }

    fn vsb(&mut self, disc: &mut Disc, file: &str, in_bar: bool) -> Option<Arc<kh2_audio::vsb::Vsb>> {
        self.vsbs
            .entry(format!("{file}{}", if in_bar { "#22" } else { "" }))
            .or_insert_with(|| {
                let raw = disc.read(file).ok()?;
                if in_bar {
                    let bar = kh2_bar::parse(&raw).ok()?;
                    kh2_audio::vsb::Vsb::parse(bar.iter().find(|e| e.kind == VOICE_BAR_ENTRY)?.data).map(Arc::new)
                } else {
                    kh2_audio::vsb::Vsb::parse(&raw).map(Arc::new)
                }
            })
            .clone()
    }

    /// kh2fm-rs `play_voices`: line `line` of a voice bank by the speaker slot's priority rule, at
    /// least `MIN_VOLUME`, centred when loud.
    fn voice(&mut self, disc: &mut Disc, speaker: &str, file: &str, in_bar: bool, line: u16, priority: u8, pos: Option<glam::Vec3>) -> bool {
        let Some(bank) = self.vsb(disc, file, in_bar) else { return false };
        let Some((sample, rate)) = bank.line(line as usize) else { return false };
        let slot = self.slots.entry(speaker.to_string()).or_default();
        if let Some(h) = slot.playing
            && !self.driver.voice_playing(kh2_audio::ssl::Handle(h))
        {
            slot.playing = None;
        }
        slot.request(line, priority);
        if slot.tick(|_| true).is_none() {
            return false;
        }
        if let Some(h) = slot.playing.take() {
            self.driver.stop_voice(kh2_audio::ssl::Handle(h));
        }
        let (v, p) = self.placement(pos);
        let volume = v.max(kh2_audio::voice::MIN_VOLUME);
        let pan = if volume > 0x39ff { kh2_audio::positional::CENTRE_PAN } else { p };
        let h = self.driver.play_voice_at(Arc::new(sample), rate, volume, pan);
        if let Some(slot) = self.slots.get_mut(speaker) {
            slot.playing = Some(h.0);
        }
        self.voices += 1;
        true
    }

    /// Starts sound (bank, index) of `model`'s or the global bank.
    pub fn play(&mut self, disc: &mut Disc, model: &str, bank: u16, index: u16) -> bool {
        self.play_at(disc, model, bank, index, None)
    }

    /// `play` placed at `pos` (cm) for the listener.
    pub fn play_at(&mut self, disc: &mut Disc, model: &str, bank: u16, index: u16, pos: Option<glam::Vec3>) -> bool {
        let own = (!model.is_empty()).then(|| self.bank(disc, &format!("obj/{model}.apdx"), false)).flatten().filter(|b| bank == OWN_BANK || b.block.bank == bank);
        let b = match own {
            Some(b) => b,
            None => match self.bank(disc, &format!("se/se{bank:03}.seb"), true) {
                Some(b) => b,
                None => {
                    self.missing.push((bank, index));
                    return false;
                }
            },
        };
        let Some(entry) = b.block.entries.get(index as usize) else {
            self.missing.push((bank, index));
            return false;
        };
        let wd = match &b.wd {
            Some(w) => Some(w.clone()),
            None => self.wd(disc, entry.wd),
        };
        let Some(wd) = wd else {
            self.missing.push((bank, index));
            return false;
        };
        let (volume, pan) = self.placement(pos);
        let ok = self.driver.play_se(&b.block, index as usize, wd, volume, pan);
        if ok {
            self.played += 1;
        }
        ok
    }

    /// Plays the waiting cues, then renders `frames` stereo frames into `out` (16-bit, interleaved).
    pub fn tick(&mut self, disc: &mut Disc, out: &mut [i16], frames: usize) {
        for c in std::mem::take(&mut self.pending) {
            match c.kind {
                Kind::Sound => {
                    let (bank, index) = (c.params[0].max(0) as u16, c.params[1].max(0) as u16);
                    self.play_at(disc, &c.model, bank, index, Some(c.pos));
                }
                Kind::Hit => {
                    // kh2-rs `PlaySe::hit`: the reaction's class picks light (0) or heavy (2), alternating per hit.
                    let base = match c.params[0] {
                        1 | 12 | 3 | 6 | 8 | 10 | 13 | 16 | 18 | 5 => Some(0u16),
                        4 | 7 | 9 | 11 | 15 | 17 => Some(2),
                        _ => None,
                    };
                    if let Some(b) = base {
                        self.hits = self.hits.wrapping_add(1);
                        self.play_at(disc, &c.model, OWN_BANK, b + (self.hits & 1) as u16, Some(c.pos));
                    }
                }
                // Voice triggers 13 / 14: for Sora a voice id the `vtbl` picks a line for (FUN_00175d18,
                // kh2fm-rs `sora_voice`; decided: the other party's NeoStatus is 0, the line -3 of a pick
                // has no other party), from `voice/<lang>/battle/<world><bank>_<speaker>.vsb`; for an
                // enemy the line of its own bank (the `.apdx` entry 0x22) with the trigger's priority.
                Kind::Voice if c.owner == crate::cue::Owner::Sora => {
                    let (Some(area), Some(world)) = (self.area.clone(), self.area.as_deref().and_then(|a| a.get(..2)).map(str::to_string)) else { continue };
                    let _ = area;
                    use kh2_audio::voice::{pick_line, resolve_line, speaker::SORA};
                    let Some(rec) = self.vtbl.iter().find(|r| r.character_id as u16 == SORA && r.id as u16 == c.params[0] as u16) else { continue };
                    let mut pairs = [0i8; 10];
                    for (p, v) in pairs.iter_mut().zip(rec.voices.iter()) {
                        *p = *v;
                    }
                    let priority = rec.priority;
                    let line = pick_line(&pairs, &mut self.rng, |l| resolve_line(l, SORA, 0, &|_| false));
                    if line < 0 {
                        continue;
                    }
                    let name = self.names.name(SORA).unwrap_or("sora").to_string();
                    let file = kh2_audio::voice::bank_path(&self.lang, &world, self.voice_bank, &name);
                    self.voice(disc, &c.model, &file, false, line as u16, priority, Some(c.pos));
                }
                Kind::Voice => {
                    let (line, priority) = (c.params[0], c.params[1].clamp(0, 255) as u8);
                    if line >= 0 && !c.model.is_empty() {
                        let file = format!("obj/{}.apdx", c.model);
                        self.voice(disc, &c.model, &file, true, line as u16, priority, Some(c.pos));
                    }
                }
                // Frame trigger 2 (kh2fm-rs `footsteps`): the area's soundinfo record, the owner's
                // NeoStatus, the ground material (`params[1]`, set by the owner), a step counter per owner.
                Kind::Footstep => {
                    let neo = match self.neo.get(&c.model) {
                        Some(n) => *n,
                        None => {
                            let n = neo_status(disc, &c.model).unwrap_or(0);
                            self.neo.insert(c.model.clone(), n);
                            n
                        }
                    };
                    let material = kh2_audio::footstep::actor_material(0, c.params[1].clamp(0, 255) as u8);
                    let counter = self.steps.entry(c.model.clone()).or_insert(0);
                    if let Some(n) = kh2_audio::footstep::footstep_number(self.soundinfo.as_ref(), neo, material, c.params[0] as u16, counter) {
                        self.play_at(disc, "", (n / 1000) as u16, (n % 1000) as u16, Some(c.pos));
                    }
                }
                Kind::SoundNumber => {
                    let n = c.params[0] as u16 as u32 | (c.params[1] as u16 as u32) << 16;
                    self.play_at(disc, "", (n / 1000) as u16, (n % 1000) as u16, None);
                }
                _ => {}
            }
        }
        if self.missing.len() > 64 {
            let extra = self.missing.len() - 64;
            self.missing.drain(..extra);
        }
        let n = frames.min(out.len() / 2);
        let mut buf = vec![0f32; n * 2];
        self.driver.render(&mut buf);
        for (o, s) in out.iter_mut().zip(buf) {
            *o = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
    }
}
