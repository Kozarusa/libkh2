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
}

impl Audio {
    pub fn new(disc: &mut Disc) -> Result<Audio, String> {
        let irx = disc.0.iso().read("LIBSSL.IRX").map_err(|e| format!("LIBSSL.IRX: {e}"))?;
        let tables = Tables::from_irx(&irx).ok_or("LIBSSL.IRX: no driver tables")?;
        let mut driver = Driver::new(Arc::new(tables));
        // The level kh2-extras renders with (the game's mixer at full level, `start_stream`).
        driver.mixer.master = 0.6;
        Ok(Audio { driver, banks: HashMap::new(), wds: HashMap::new(), pending: Vec::new(), missing: Vec::new(), played: 0, listener: None, hits: 0 })
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
