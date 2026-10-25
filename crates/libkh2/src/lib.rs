#[path = "../../../../kh2-rs/crates/kh2-player/src/controller_rules.rs"]
pub mod controller_rules;
pub fn x() -> f32 { kh2_tables::tables::PREF_PRTY.record_size.unwrap_or(0) as f32 + controller_rules::WALK_LIMIT }
