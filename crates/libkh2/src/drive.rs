//! Drive forms: enter, the change motions, the body swap, the form timer and revert.
//!
//! The rules are kh2-rs's `shared::drive::rules` (compiled from kh2-rs's text,
//! `kh2_game_core::drive_rules`); the flow transcribes kh2-rs `kh2-player/src/drive.rs::drive_tick`
//! (`DriveRequest::Enter`, the change task, the form timer, `do_revert`) and
//! `DriveRules::object_files` without Bevy.

use std::sync::Arc;

use kh2_anim::moveset::Moveset;
use kh2_game_core::drive_rules as rules;
use kh2_tables::player::{self, DriveForm, FormLevels, Memt};
use kh2_tables::tables as t;

use crate::anim::Rig;
use crate::data::{AttackLeaps, Disc, LoadError};

/// The change motions (kh2-rs `drive::MOTION_LEAVE`, `MOTION_ARRIVE`) and the change task's timeout.
pub const MOTION_LEAVE: usize = 52;
pub const MOTION_ARRIVE: usize = 53;
pub const CHANGE_TIMEOUT: u32 = 600;

/// kh2-rs `shared::drive::DriveRules`: the tables the drive code reads.
pub struct DriveData {
    pub forms: Vec<DriveForm>,
    pub levels: FormLevels,
    pub tuning: rules::Tuning,
    pub memt: Memt,
    pub objs: Vec<t::Objentry>,
    pub items: Vec<t::Item>,
    pub wmst: Vec<[String; 2]>,
    pub went: Vec<u8>,
    pub key_item: u16,
}

/// What a form's object is made of (kh2-rs `FormFiles`, ids added for the host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormFiles {
    pub object: u16,
    pub model: String,
    pub mset: String,
    pub moveset: u16,
    /// Per hand: (weapon object id, model, motion set).
    pub hands: [Option<(u32, String, String)>; 2],
}

/// A form's loaded body (kh2-rs `FormAssets`).
pub struct FormAssets {
    pub files: FormFiles,
    pub body: Arc<Rig>,
    pub weapon: Option<Arc<Rig>>,
    /// The second hand's keyblade (Valor, Master, Final), on bone `HAND_BONES[1]`.
    pub weapon2: Option<Arc<Rig>>,
    pub moves: Arc<Moveset>,
    pub leaps: Arc<AttackLeaps>,
}

impl DriveData {
    pub fn load(d: &mut Disc) -> Result<DriveData, LoadError> {
        let a = &mut d.0;
        fn e(w: &'static str) -> impl Fn(kh2_tables::DecodeError) -> LoadError {
            move |x| LoadError(format!("{w}: {x}"))
        }
        let items = kh2_tables::load::<t::Item>(a, &t::ITEM).map_err(e("item"))?.rows;
        let cmds = kh2_tables::load::<t::Cmd>(a, &t::CMD).map_err(e("cmd"))?.rows;
        let fmlv = kh2_tables::load::<t::Fmlv>(a, &t::FMLV).map_err(e("fmlv"))?.rows;
        let objs = kh2_tables::load::<t::Objentry>(a, &t::OBJENTRY).map_err(e("objentry"))?.rows;
        let memt = player::load_memt(a).map_err(e("memt"))?;
        let sstm = player::load_sstm(a).map_err(e("sstm"))?;
        let wmst = rules::parse_wmst(&kh2_tables::subfile(a, &t::WMST).map_err(e("wmst"))?);
        let went = kh2_tables::subfile(a, &t::WENT).map_err(e("went"))?;
        Ok(DriveData { forms: player::drive_forms(&items, &cmds, &memt), levels: FormLevels::from_rows(&fmlv), tuning: rules::Tuning::from_sstm(&sstm), memt, objs, items, wmst, went, key_item: rules::KINGDOM_KEY })
    }

    pub fn form(&self, form: u8) -> Option<&DriveForm> {
        self.forms.iter().find(|f| f.form == form)
    }

    fn item_slot(&self, item: u16) -> Option<u16> {
        self.items.iter().find(|i| i.id == item).map(|i| i.flag1 as u16 | (i.rank as u16) << 8)
    }

    /// kh2-rs `DriveRules::object_files`.
    pub fn object_files(&self, obj: u16, hand1_item: u16) -> Option<FormFiles> {
        let o = self.objs.iter().find(|o| o.object_id == obj as u32)?;
        let ms = o.neo_moveset as usize;
        let mset_of = |hand: usize| self.wmst.get(ms).and_then(|n| n.get(hand)).filter(|s| !s.is_empty()).cloned();
        let weapon = |item: u16, hand: usize| {
            let id = rules::weapon_object(&self.went, ms, self.item_slot(item)?)?;
            let w = self.objs.iter().find(|w| w.object_id == id)?;
            Some((id, w.model_name.clone(), mset_of(hand)?))
        };
        Some(FormFiles {
            object: obj,
            model: o.model_name.clone(),
            mset: o.animation_name.trim_end_matches(".mset").to_string(),
            moveset: o.neo_moveset,
            hands: [weapon(self.key_item, 0), (hand1_item != 0).then(|| weapon(hand1_item, 1)).flatten()],
        })
    }

    /// kh2-rs `DriveRules::files`: a form's files in world `world` (memt merge, no story flags) with
    /// the keyblade item of the second hand (kh2-rs `rules::take_form_keyblade`: Valor, Master and
    /// Final take one; 0 for the others).
    pub fn files(&self, form: u8, world: u16, hand1_item: u16) -> Option<FormFiles> {
        let rec = rules::memt_merge(&self.memt, world, |_| false);
        let obj = rules::merged_form_object(&rec, form)?;
        self.object_files(obj, if matches!(form, 1 | 3 | 4) { hand1_item } else { 0 })
    }

    /// The keyblade items (item type 2), for the host's choice of the second blade.
    pub fn keyblades(&self) -> Vec<u16> {
        self.items.iter().filter(|i| i.r#type == rules::KEYBLADE_TYPE).map(|i| i.id).collect()
    }
}

impl Disc {
    /// kh2-rs `drive::load_assets`: the form's body, its hand-0 weapon and its attack table.
    /// `hand1_item` is the keyblade item of the second hand (decided: the host picks it; kh2-rs takes
    /// the first keyblade of the inventory; `kh2_drive_enter` defaults to the Kingdom Key).
    pub fn form_assets(&mut self, data: &DriveData, form: u8, world: u16, hand1_item: u16) -> Result<FormAssets, LoadError> {
        let files = data.files(form, world, hand1_item).ok_or_else(|| LoadError(format!("form {form}: no object")))?;
        let body = Arc::new(self.rig_named(&files.model, &files.mset)?);
        let mut hand = |h: usize| match &files.hands[h] {
            Some((_, m, s)) => self.rig_named(m, s).ok().map(Arc::new),
            None => None,
        };
        let (weapon, weapon2) = (hand(0), hand(1));
        let ptya = kh2_tables::subfile(&mut self.0, &t::PTYA).map_err(|e| LoadError(format!("ptya: {e}")))?;
        let moves = Moveset::parse(&ptya, files.moveset as usize).unwrap_or_default();
        let leaps = AttackLeaps::parse(&ptya, files.moveset as usize);
        Ok(FormAssets { files, body, weapon, weapon2, moves: Arc::new(moves), leaps: Arc::new(leaps) })
    }
}

/// The change in progress (kh2-rs `ChangeTask`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Leaving,
    Arriving,
}

/// Sora's drive state (kh2-rs `DriveState`, the parts the flow uses).
#[derive(Clone, Debug)]
pub struct DriveState {
    pub points: rules::Points,
    pub form: u8,
    pub in_form: bool,
    /// Form levels by form id, and which forms are owned.
    pub level: [u8; 6],
    pub owned: [bool; 6],
    pub timer: rules::FormTimer,
    pub counters: rules::Counters,
    pub rng: rules::Lcg,
    pub donald: bool,
    pub goofy: bool,
    /// (target form, phase, ticks, a change motion plays).
    pub change: Option<(u8, Phase, u32, bool)>,
    /// The form's files (object and weapon ids for the host).
    pub files: Option<FormFiles>,
}

impl Default for DriveState {
    fn default() -> DriveState {
        DriveState {
            points: rules::Points { whole: 0, fraction: 0, max: 3 },
            form: 0,
            in_form: false,
            level: [1; 6],
            owned: [false; 6],
            timer: Default::default(),
            counters: Default::default(),
            rng: rules::Lcg(1),
            donald: true,
            goofy: true,
            change: None,
            files: None,
        }
    }
}
