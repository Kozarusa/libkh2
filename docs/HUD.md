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

Sora's HP / MP panel and drive gauge (kh2-rs `hud_gauge`), the party panels (`hud_party`), the
target gauge and lock-on reticle (`hud_target`), banners (`hud_banner`), the INFORMATION bar
(`hud_info`), the message window, the mission panels, the clip / scissor those need, the ring
wedges. Each is a transcription of its kh2-rs system onto the same node / draw-list core.
