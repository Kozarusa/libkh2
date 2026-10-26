# The field HUD

Is a HUD possible? Yes: kh2-rs has the field HUD ported (`kh2-hud`, `kh2-game-core`'s `hud_gfx`
and `hud_text`), and its core is pure: `kh2_image` plays the game's SEQD sequences into quads and
lays the game's fonts out into glyph quads; the layout rules of each panel are small functions
over constants and tables read from the executable. The Bevy part (scene nodes as entities,
meshes, materials, cameras) is what libkh2 replaces with a draw list.

## What libkh2 draws (round 10)

The command menu, bottom left, as the game builds it (kh2-rs `hud_menu`, compiled from source:
the group tables at 0x3551c0.., the colours at 0x355600 / 0x355628, `page_x`, `row_y`,
`header_y`, `capsule_y`, `bar_group`, `icon_group`, `base_entries`):

- page 0: the four basic commands (Attack, Magic, Items, Drive) under the COMMANDS banner, the
  selected bar with its medallion, icon and arrow;
- Magic opens the spells at their levels (an MP-short spell's name orange, style 2), Items the
  host's list (`kh2_hud_set_items`), Drive the owned forms (short of drive points: style 5);
- the reaction capsule above the list, with the offered reaction command's name.

Input: `kh2_hud_input` with `KH2_HUD_UP / DOWN / CONFIRM / BACK / REACTION`. Choices: Attack
presses attack on Sora's next tick, a spell starts the cast (`kh2_cast`'s path), the reaction
command is accepted (`kh2_reaction_accept`'s path); items and drive forms are reported
(`kh2_hud_poll_event`) for the host to call `kh2_use_item` / `kh2_drive_enter`.

## Sora's HP / MP panel and drive gauge (1.2)

Bottom right, from the world's field file (`field2d/us/<world>0field.2dd`, pair `gaug`) and Sora's
face (the last pair of `obj/P_EX100.apdx`), compiled from kh2-rs `hud_gauge` (group numbers,
`PanelState`, `PanelEvent`, `fill`, `DriveTables` from the executable, `alarm_step`) with the
panel's frame transcribed: the HP ring fills pi/40 per point up to 60 HP and a bar beyond (3 px a
point), three layers (maximum, the trail of the last hit, the current HP); a hit flashes the circle
and shows the hurt face once; HP at or below a quarter turns the panel to its danger look and
starts the danger alarm (a numbered sound cue 9); the MP bar is a pixel a point (the recharge look
while the MP charge runs); the drive gauge shows the whole points as a digit, the fraction as the
fill, MAX when full, the form timer's segment in a form. The values are Sora's own each tick.

Node features this needed (kh2-rs `hud_gfx`): parents with the sequence anchors, offsets, the
sequence's external scale and rotation, the ring wedge clip (`wedge`) and the fill reveal; a
clipped quad comes out of the draw list as triangles (`pos[3] == pos[2]`). Screen y includes
kh2-rs's `Y_BIAS` (16).

The viewer draws the HUD over its view (`cargo run --release -p libkh2-viewer`;
`KH2_VIEWER_FRAMES=200 KH2_VIEWER_SHOT=local/viewer_battle.png` saves a battle frame: the command
deck with the Reversal capsule, the HP ring, MP and the drive gauge at MAX 9). The disc test
`the_hp_panel_follows_sora` saves `local/hud_gauge.png` (15 of 40 HP after a hit, half MP, 2.5
drive points).

## The target gauge and the lock-on reticle (1.2)

Top right and on the target, compiled from kh2-rs `hud_target` (groups, `lock_group`, `frame_shown`, `SEGMENT`, `PIPS`) with its frame transcribed: the locked enemy (the one nearest Sora's lock-on point), else the enemy that lost HP last for `HOLD_TICKS` (300), gets the gauge (200 HP a segment, the trail of the last hit, a pip per segment below); the reticle (group 0x1b) sits at the locked enemy's point (feet + 100 cm, decided: kh2-rs takes the first target shape's centre) projected with the library's camera (`kh2_camera_tick`'s view; `Hud::observe` / `project` for Rust hosts with their own camera). `the_target_gauge_and_reticle_follow_the_lock`; the viewer's battle frame shows both.

## The draw list

`kh2_hud_draw_list` gives `kh2_hud_sprite`s in draw order: texture id, blend (0 alpha, 1
additive, 2 subtractive), nearest filtering, the four corners on the game's 512 x 448 screen (y
down; the game's HUD y 0 is 16 lines below the picture's top, kh2-rs `Y_BIAS`), their texel
coordinates (in texels of the texture, divide by its size), RGBA per corner where 0x80 = 1.0
(draw `texel * colour / 128`, alpha too). Text glyphs are sprites with `text` = 1, from the font
atlas pages (shadows first). There is no clip rectangle in what is drawn today.

Textures: `kh2_hud_texture_info` / `kh2_hud_texture_get`: the menu art (`field2d/us/<world>0command.2dd`,
`zz` as kh2-rs's fallback), the font atlas pages (`msg/us/fontimage.bar`, the PS2 palette rows as
the game samples them), the icons. RGBA8, straight alpha, the PS2 texel values (alpha 0x80 =
opaque).

## Seeing it

The disc test `the_command_menu_draws_and_casts_fire` rasterises the draw list in software
(`Hud::rasterize`) and writes `local/hud_frame.png` and `local/hud_frame_magic.png` (git-ignored):
the COMMANDS deck with Attack selected, then the MAGIC page with Fire, Blizzard, Thunder and Cure.

## Not drawn yet (TODO.md)

The party panels (`hud_party`), banners (`hud_banner`), the INFORMATION bar (`hud_info`), the
message and tutorial windows (`hud_msgwin`, `hud_tutorial`), the area-name card (`hud_card`), the
mission panels (`hud_mission`), the magic lock-on marker. kh2-rs has no damage or EXP popups of
the original (its popups are a developer overlay). Each is a transcription of its kh2-rs system
onto the same node / draw-list core.

## Final Mix (2.1.0)

- Text: the Final Mix disc's `msg/jp` font files (read through the `msg/us` names) are the Japanese
  font set: two planes per atlas and Japanese cells (`Script::Japanese`, kh2fm-rs `FontSet::for_text`
  without the English import). The glyph's page picks the plane and the palette row (kh2fm-rs
  `AtlasKey::of`). The menu reads たたかう / まほう / アイテム / ドライブ as on the disc.
- Party panels (`Hud::party`, filled by `kh2_hud_tick` from the followers): kh2fm-rs `hud_party`'s
  circle 71, rings 64 / 65 / 66 with the HP and MP wedge rules and each member's own `face` pair.
  Their place is the circle group's own anchor. Checked against kh2fm-rs's HUD (2.2.0): its party
  circle is the same node (gauge group 71, a later slot `unk20` below) whose only extra offset is the
  `Edge::Right` margin, which kh2fm-rs's own notes call the user's widescreen anchoring, "not a rule of
  the original", and which is 0 at 4:3 (`KH2_HUD_43`). So the 4:3 place is the anchor, as here; in the
  512 x 448 frame the circles reach past the right edge as the anchor puts them (a host drawing wider
  than 4:3 may shift right-edge trees by its margin, as kh2fm-rs does).
- Area-name card (`kh2_hud_area_card`): kh2fm-rs `hud_card` (USA 0x15ade8 / 0x15a640 / 0x15a840),
  the place table's name in font 2 at (256, 192), 120 frames.
- INFORMATION bar (2.2.0, `kh2_hud_info`): kh2fm-rs `hud_info` (gauge groups 0 -> 1 and 0 -> 2,
  font 0 colour 0x80e5e5e5, a 180-tick wait, 0.5 units a tick of scroll, group 3 to close); frame in
  `local/hud_info.png`. decided: no text scissor (the host clips at the screen edge).
- Message window (2.3.0, `kh2_hud_message` / `kh2_hud_message_confirm`): kh2fm-rs `hud_msgwin` with
  kh2_image's window rules (the four classes, `place`, `text_box`, the frame stretched around the
  text): open group, typing, prompt 0x41, the button, close groups; frames `local/hud_window_<n>.png`.
  decided: one page (the 0x10 page split is not done), no speech tail, no pointer, no selection slots.
  The frame image lives in the world message files (`msg/<lang>/<world>.bar`, entry `md_m`), which
  the archive only finds by name: set `KH2_NAMES` to a name list (OpenKH's `kh2idx.txt`; libkh2 ships
  none). With it the world's own command art is found too (Twilight Town's frame); without it the
  window is unavailable (`kh2_hud_message` returns 0) and the menu uses the shared `zz` art.
- Not done: banners and experience popups (`hud_banner`), tutorial windows (`hud_tutorial`), mission
  panels (`hud_mission`).

