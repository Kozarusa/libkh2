# Runtime meshes and textures

Like libsm64 hands the host Mario's triangles every frame, libkh2 hands it the skinned mesh of any
body it runs, so a host can draw Sora, Donald, Goofy, the enemies and the summon with no exporter.

## Calls

```c
kh2_mesh_info info;
kh2_mesh_prepare(h, 0 /* Sora */, 0 /* body */, 1 /* world space */, &info);
float *pos = malloc(12 * info.vertex_count), *nrm = malloc(12 * info.vertex_count);
float *uv = malloc(8 * info.vertex_count), *col = malloc(16 * info.vertex_count);
uint32_t *idx = malloc(4 * info.index_count);
kh2_submesh *sub = malloc(sizeof *sub * info.submesh_count);
kh2_mesh_get(pos, nrm, uv, col, info.vertex_count, idx, info.index_count, sub, info.submesh_count);
for (uint32_t k = 0; k < info.submesh_count; k++) {
    /* bind texture sub[k].texture (kh2_texture_info / kh2_texture_get, upload once per id),
       draw idx[sub[k].first_index .. + sub[k].index_count] as triangles;
       transparent submeshes after the opaque ones, alpha-blended */
}
```

Owners: 0 Sora (`part` 0 body, 1 keyblade, 2 second keyblade), 1 a member (`part` = follower slot),
2 an enemy (`part` = index in `kh2_enemy_state` order), 3 the summon.

## Buffer layout

| buffer | per vertex | meaning |
|---|---|---|
| positions | 3 floats | cm, library space (right-handed, Y up), world when `space` = 1 |
| normals | 3 floats | unit; the area-weighted average of the skinned triangles' normals (the model files carry none) |
| uvs | 2 floats | texture coordinates (repeat / clamp as the texture says; region clamps are not applied) |
| colors | 4 floats | the PS2 vertex colour, 1.0 = neutral (0x80); the game's look is texture times twice this |
| indices | u32 | triangle list into the vertices; one run per submesh |

Textures: `kh2_texture_get` writes RGBA8, row-major from the top, straight alpha; the PS2's alpha
range 0..0x80 is mapped to 0..255 by kh2-rs's decoder (`kh2_model`, CLUT and swizzle decoded).
Texture ids are global and stay valid for the library's life (a texture keeps its model loaded).

decided: a texture table (one image per id) instead of a packed atlas or array: KH2 textures have
many sizes; a host builds its own atlas or array from them if it wants one.

## Skinning

`kh2_model` decodes the model's VIF strips with the vertices in the rest pose and up to four
(bone, weight) pairs. libkh2 skins each vertex as `sum w * (root * pose[j] * inverse(rest[j])) * v`
with the body's current pose (the same matrices `kh2_get_bones` returns). Sora's body: 4,271
vertices, 2,925 triangles, 7 submeshes, 5 textures.

## Unity

`unity/Kh2Mesh.cs` fills a Unity `Mesh` from these buffers every frame (x mirrored, metres, winding
flipped) and makes one `Texture2D` per texture id. It is written against the Unity API and is not
built here (no UnityEngine on this machine); the bindings it calls are in `Kh2.cs`, which
`examples/csharp` compiles and runs.
