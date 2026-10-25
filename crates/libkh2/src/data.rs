//! Everything libkh2 reads from the user's disc at start-up: Sora's movement rows of `pref`
//! (`03system.bin`), his model and motion set, the keyblade's, his attack table (`ptya` of
//! `00battle.bin`, picked by his `00objentry` row) and a few constants of the executable. Nothing
//! here is a game value; the values arrive at run time.

use std::path::Path;
use std::sync::Arc;

use kh2_anim::moveset::{self, Action, Moveset};
use kh2_game_core::reaction_rules::{self as rules, ExeConsts, PatnRow, ReactionDefaults};
use kh2_tables::tables::{self as t, Atkp, Cmd, Magc, PrefFmab, PrefPlyr, PrefPrty, PrefSstm};

use crate::anim::Rig;

/// Index of Sora's row in `prty` and `plyr` (kh2-rs `data::SORA_PREF`).
const SORA_PREF: usize = 1;

/// The model and motion set names of Sora's base body and his keyblade (kh2-rs `SoraPlugin`).
pub const SORA_MODEL: &str = "P_EX100";
pub const KEYBLADE_MODEL: &str = "W_EX010";

/// The executable's constants of the attack turns (kh2-rs `shared::sora::SwingConsts`, the two
/// fields libkh2 uses).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwingConsts {
    /// DAT_0037a970: no turn while the target is this close (horizontal units).
    pub turn_min_gap: f32,
    /// DAT_0037a988: the trigger parameter's unit for the turn rate.
    pub turn_unit: f32,
}

impl SwingConsts {
    pub fn from_exe(exe: &[u8]) -> Option<SwingConsts> {
        let f = |va: u32| kh2_game_core::reaction_rules::exe_f32(exe, va);
        Some(SwingConsts { turn_min_gap: f(0x37a970)?, turn_unit: f(0x37a988)? })
    }
}

/// Sora's movement and attack parameters.
#[derive(Clone, Debug)]
pub struct Params {
    pub prty: PrefPrty,
    pub plyr: PrefPlyr,
    pub sstm: PrefSstm,
    /// Growth-ability rows, row = level - 1.
    pub fmab: Vec<PrefFmab>,
    pub swing: Option<SwingConsts>,
    /// The reaction rules' data (kh2-rs `combat::ReactionData`); `None` without the executable.
    pub reaction: Option<Reaction>,
    /// The attack rows (`atkp` of `00battle.bin`).
    pub atkp: Vec<Atkp>,
    /// Sora's maximum HP at level 1 (`plrp` HP of character 1, kh2-rs `GameData::sora_max_hp`).
    pub max_hp: i32,
    /// Sora's maximum MP (`plrp` MP).
    pub max_mp: i32,
    /// The field camera's numbers from the executable (`camera_rules::Params::from_exe`).
    pub camera: Option<kh2_game_core::camera_rules::Params>,
    /// The spell rows (`magc` of `00battle.bin`) and the commands (`cmd` of `03system.bin`).
    pub magc: Vec<Magc>,
    pub cmd: Vec<Cmd>,
    /// The spells' size rows (`magi` of the `pref` entry of `03system.bin`).
    pub magi: Vec<kh2_game_core::magic_rules::MagiRow>,
    /// Sora's level rows (`lvup`, character 0), and the Kingdom Key's attack and magic
    /// (`item_stat`; kh2-rs `GameData::weapon_attack` / `weapon_magic`).
    pub levels: Vec<t::Lvup>,
    pub weapon_attack: i32,
    pub weapon_magic: i32,
}

/// kh2-rs `combat::ReactionData`: the executable's constants, the `sstm` defaults, the `patn` table.
#[derive(Clone, Debug)]
pub struct Reaction {
    pub consts: ExeConsts,
    pub defaults: ReactionDefaults,
    pub patn: Vec<PatnRow>,
}

impl Params {
    /// kh2-rs `GameData::fmab`.
    pub fn fmab(&self, level: u8) -> Option<&PrefFmab> {
        (level > 0).then(|| self.fmab.get(level as usize - 1)).flatten()
    }

    /// kh2-rs `GameData::atkp`: sub-id 0 first.
    pub fn atkp(&self, id: i16) -> Option<&Atkp> {
        let id = id as u16;
        self.atkp.iter().find(|a| a.id == id && a.sub_id >> 8 == 0).or_else(|| self.atkp.iter().find(|a| a.id == id))
    }
}

/// The leap of an attack record (kh2-rs `sora::Leap`, the record fields FUN_00176050 reads).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Leap {
    pub height: f32,
    pub reach: f32,
    pub min_height: f32,
    pub min_speed: f32,
    pub max_speed: f32,
    pub gap: f32,
}

impl Leap {
    fn from_record(r: &[u8]) -> Option<Leap> {
        let f = |o: usize| r.get(o..o + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
        Some(Leap { height: f(0xc)?, reach: f(0x10)?, min_height: f(0x14)?, min_speed: f(0x18)?, max_speed: f(0x1c)?, gap: f(0x3c)? })
    }
}

/// The leap fields of every record of one `ptya` table (kh2-rs `sora::AttackLeaps`, one table).
#[derive(Clone, Debug, Default)]
pub struct AttackLeaps(pub Vec<(Action, Leap)>);

impl AttackLeaps {
    pub fn parse(ptya: &[u8], index: usize) -> AttackLeaps {
        let word = |o: usize| ptya.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize);
        let mut out = Vec::new();
        if let (Some(m), Some(at)) = (Moveset::parse(ptya, index), word(8 + 4 * index)) {
            for (i, a) in m.actions.into_iter().enumerate() {
                let start = at + 4 + moveset::RECORD_SIZE * i;
                if let Some(l) = ptya.get(start..start + moveset::RECORD_SIZE).and_then(Leap::from_record) {
                    out.push((a, l));
                }
            }
        }
        AttackLeaps(out)
    }

    /// The leap of a started record (all zero when the record is not found).
    pub fn of(&self, a: &Action) -> Leap {
        self.0.iter().find(|(x, _)| x == a).map_or(Leap::default(), |(_, l)| *l)
    }
}

/// Everything a Sora instance shares with the others.
pub struct Game {
    pub params: Params,
    pub sora: Arc<Rig>,
    pub keyblade: Option<Arc<Rig>>,
    pub moves: Arc<Moveset>,
    pub leaps: AttackLeaps,
    /// The drive tables (`None` when they could not be read).
    pub drive: Option<Arc<crate::drive::DriveData>>,
    /// The ability rules (kh2-rs `shared::abilities::AbilityRules`); `None` when not readable.
    pub ability_rules: Option<Arc<kh2_game_core::abilities::AbilityRules>>,
}

#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

fn err<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> LoadError + '_ {
    move |e| LoadError(format!("{what}: {e}"))
}

/// The opened disc.
pub struct Disc(pub kh2_archive::Archive);

impl Disc {
    pub fn open(iso: &Path) -> Result<Disc, LoadError> {
        kh2_archive::Archive::open(iso).map(Disc).map_err(err("opening the disc"))
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, LoadError> {
        self.0.read(name).map_err(err(name))
    }

    /// The boot executable (`SLUS_210.05` on the USA disc).
    pub fn exe(&mut self) -> Option<Vec<u8>> {
        let name = self.0.boot_executable().to_string();
        self.0.iso().read(&name).ok()
    }

    /// An area's collision (`map/us/<area>.map`, its COCT). The world's sub-index is named after
    /// the area's first two letters (`000tt.idx` for `tt00`) and is expanded on first use.
    pub fn coct(&mut self, area: &str) -> Result<kh2_map::Coct, LoadError> {
        let world: String = area.chars().take(2).collect();
        let _ = self.0.apply_names([format!("000{world}.idx").as_str()]);
        let path = format!("map/us/{area}.map");
        let map = self.read(&path)?;
        kh2_map::Coct::from_map(&map).ok_or_else(|| LoadError(format!("{path}: no collision")))
    }

    fn rig(&mut self, name: &str) -> Result<Rig, LoadError> {
        self.rig_named(name, name)
    }

    /// A model and a motion set by object names (`obj/<model>.mdlx`, `obj/<mset>.mset`).
    pub fn rig_named(&mut self, name: &str, mset_name: &str) -> Result<Rig, LoadError> {
        let model = self.read(&format!("obj/{name}.mdlx")).and_then(|d| kh2_model::mdlx(&d).ok_or_else(|| LoadError(format!("{name}.mdlx: not a model"))))?;
        let mset = self.read(&format!("obj/{mset_name}.mset")).and_then(|d| kh2_anim::Mset::parse(&d).ok_or_else(|| LoadError(format!("{name}.mset: not a motion set"))))?;
        Ok(Rig { model, mset })
    }

    fn table<T: kh2_tables::Record>(&mut self, def: &kh2_tables::TableDef) -> Result<Vec<T>, LoadError> {
        kh2_tables::load::<T>(&mut self.0, def).map(|d| d.rows).map_err(err(def.name))
    }

    pub fn params(&mut self) -> Result<Params, LoadError> {
        fn pick<T>(v: Vec<T>, i: usize, what: &str) -> Result<T, LoadError> {
            v.into_iter().nth(i).ok_or_else(|| LoadError(format!("{what}: no row {i}")))
        }
        let prty = pick(self.table::<PrefPrty>(&t::PREF_PRTY)?, SORA_PREF, "pref prty")?;
        let plyr = pick(self.table::<PrefPlyr>(&t::PREF_PLYR)?, SORA_PREF, "pref plyr")?;
        let sstm = pick(self.table::<PrefSstm>(&t::PREF_SSTM)?, 0, "pref sstm")?;
        let fmab = self.table::<PrefFmab>(&t::PREF_FMAB)?;
        let exe = self.exe();
        let swing = exe.as_deref().and_then(SwingConsts::from_exe);
        let patn = kh2_tables::subfile(&mut self.0, &t::PATN).ok().and_then(|b| rules::parse_patn(&b)).unwrap_or_default();
        let reaction = exe.as_deref().and_then(ExeConsts::from_exe).map(|consts| Reaction { consts, defaults: ReactionDefaults::from(&sstm), patn });
        let atkp = self.table::<Atkp>(&t::ATKP)?;
        let plrp = self.table::<t::Plrp>(&t::PLRP)?;
        let sora = plrp.iter().find(|p| p.character == 1);
        let (max_hp, max_mp) = (sora.map_or(20, |p| p.hp as i32), sora.map_or(100, |p| p.mp as i32));
        let camera = exe.as_deref().and_then(kh2_game_core::camera_rules::Params::from_exe);
        let magc = self.table::<Magc>(&t::MAGC).unwrap_or_default();
        let cmd = self.table::<Cmd>(&t::CMD).unwrap_or_default();
        let magi = self
            .read("03system.bin")
            .ok()
            .and_then(|f| {
                let top = kh2_bar::parse(&f).ok()?;
                let inner = kh2_bar::parse(kh2_bar::find(&top, "pref")?.data).ok()?;
                Some(kh2_game_core::magic_rules::parse_magi(kh2_bar::find(&inner, "magi")?.data))
            })
            .unwrap_or_default();
        let levels = kh2_tables::subfile(&mut self.0, &t::LVUP).ok().and_then(|b| kh2_tables::rt::decode_lvup::<t::Lvup>(&b)).and_then(|c| c.into_iter().next()).unwrap_or_default();
        let items = self.table::<t::Item>(&t::ITEM).unwrap_or_default();
        let stats = self.table::<t::ItemStat>(&t::ITEM_STAT).unwrap_or_default();
        let kk = items.iter().find(|i| i.id == 41).and_then(|i| stats.iter().find(|s| s.id == i.stat_entry));
        let (weapon_attack, weapon_magic) = kk.map_or((0, 0), |s| (s.attack as i32, s.magic as i32));
        Ok(Params { prty, plyr, sstm, fmab, swing, reaction, atkp, max_hp, max_mp, camera, magc, cmd, magi, levels, weapon_attack, weapon_magic })
    }

    /// Everything Sora needs.
    pub fn game(&mut self) -> Result<Game, LoadError> {
        let params = self.params()?;
        let sora = Arc::new(self.rig(SORA_MODEL)?);
        let keyblade = self.rig(KEYBLADE_MODEL).ok().map(Arc::new);
        // kh2-rs `sora::load_moveset`: the attack table is the `NeoMoveset` of the model's objentry row.
        let index = self
            .table::<t::Objentry>(&t::OBJENTRY)?
            .into_iter()
            .find(|r| r.model_name == SORA_MODEL)
            .map(|r| r.neo_moveset as usize)
            .ok_or_else(|| LoadError(format!("objentry: no {SORA_MODEL} row")))?;
        let ptya = kh2_tables::subfile(&mut self.0, &t::PTYA).map_err(err("ptya"))?;
        let moves = Moveset::parse(&ptya, index).ok_or_else(|| LoadError(format!("ptya: no table {index}")))?;
        let leaps = AttackLeaps::parse(&ptya, index);
        let drive = crate::drive::DriveData::load(self).ok().map(Arc::new);
        let ability_rules = kh2_game_core::abilities::AbilityRules::load(&mut self.0).map(Arc::new);
        Ok(Game { params, sora, keyblade, moves: Arc::new(moves), leaps, drive, ability_rules })
    }
}
