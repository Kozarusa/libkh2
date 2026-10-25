//! Items and equipment.
//!
//! The rules are kh2-rs's (`data.rs` `GameData::sora_status`, `fold_resistances`; `shared/party.rs`
//! `MemberStatus::member_status` / `member_ap_resistances`): every equipped item's `item_stat` row
//! (FUN_001a0c08) adds its attack, magic, defense and AP to the status, and its resistance bytes
//! scale the status's resistances in order, `ceil(res x row / 100)`. An equipped item's ability
//! (`item_stat` +2, a keyblade's or an accessory's) is equipped with it.
//!
//! The inventory is the host's: the library answers what an item is (`info`).

use kh2_tables::tables as t;

/// kh2-rs `fold_resistances`: from 100 each element, scaled by every row in order.
pub fn fold_resistances<'a>(rows: impl Iterator<Item = &'a [i32; 8]>) -> [i32; 7] {
    rows.fold([100i32; 7], |w, e| std::array::from_fn(|k| (w[k] * e[1 + k] + 99) / 100))
}

/// The `item_stat` row of an item.
pub fn stat_of<'a>(items: &[t::Item], stats: &'a [t::ItemStat], id: u16) -> Option<&'a t::ItemStat> {
    let i = items.iter().find(|i| i.id == id && id != 0)?;
    stats.iter().find(|s| s.id == i.stat_entry)
}

/// The equipment's sum: (attack, magic, defense), AP, resistances.
pub fn gear(items: &[t::Item], stats: &[t::ItemStat], equipped: &[u16]) -> ([i32; 3], i32, [i32; 7]) {
    let rows: Vec<&t::ItemStat> = equipped.iter().filter_map(|&id| stat_of(items, stats, id)).collect();
    let g = rows.iter().fold([0; 3], |a, s| [a[0] + i32::from(s.attack), a[1] + i32::from(s.magic), a[2] + i32::from(s.defense)]);
    let ap = rows.iter().map(|s| i32::from(s.ability_points)).sum();
    let res: Vec<[i32; 8]> = rows.iter().map(|s| [s.ability_points, s.unknown08, s.fire_resistance, s.ice_resistance, s.lightning_resistance, s.dark_resistance, s.unknown0d, s.general_resistance].map(i32::from)).collect();
    (g, ap, fold_resistances(res.iter()))
}

/// The abilities the equipment carries (`item_stat` +2, 0 none).
pub fn abilities(items: &[t::Item], stats: &[t::ItemStat], equipped: &[u16]) -> Vec<u16> {
    equipped.iter().filter_map(|&id| stat_of(items, stats, id)).map(|s| s.ability).filter(|&a| a != 0).collect()
}

/// What an item is (the `item` row and its `item_stat` row).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemInfo {
    pub id: u16,
    /// `item.Type` (2 keyblade, 3 staff, 4 shield, 19 ability, ...).
    pub kind: u8,
    /// The `cmd` row it uses (consumables), its name and description message ids.
    pub command: u16,
    pub name: u16,
    pub description: u16,
    /// `Flag1 | Rank << 8`: a consumable's amount, an ability's bit.
    pub amount: u16,
    pub shop_buy: u16,
    pub shop_sell: u16,
    /// The `item_stat` row: attack, magic, defense, AP, resistances (physical, fire, blizzard, thunder,
    /// dark, light, general), the ability it carries.
    pub attack: u8,
    pub magic: u8,
    pub defense: u8,
    pub ap: u8,
    pub resist: [u8; 7],
    pub ability: u16,
}

pub fn info(items: &[t::Item], stats: &[t::ItemStat], id: u16) -> Option<ItemInfo> {
    let i = items.iter().find(|i| i.id == id)?;
    let s = stats.iter().find(|s| s.id == i.stat_entry);
    Some(ItemInfo {
        id,
        kind: i.r#type,
        command: i.command,
        name: i.name,
        description: i.description,
        amount: i.flag1 as u16 | (i.rank as u16) << 8,
        shop_buy: i.shop_buy,
        shop_sell: i.shop_sell,
        attack: s.map_or(0, |s| s.attack),
        magic: s.map_or(0, |s| s.magic),
        defense: s.map_or(0, |s| s.defense),
        ap: s.map_or(0, |s| s.ability_points),
        resist: s.map_or([0; 7], |s| [s.unknown08, s.fire_resistance, s.ice_resistance, s.lightning_resistance, s.dark_resistance, s.unknown0d, s.general_resistance]),
        ability: s.map_or(0, |s| s.ability),
    })
}
