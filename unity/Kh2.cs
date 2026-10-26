// libkh2 for Unity: P/Invoke bindings and a small MonoBehaviour driver.
//
// Not built or run in this repository; it documents how a Unity host uses the library.
//
// Setup
//   1. cargo build --release -p libkh2, copy target/release/kh2.dll (Windows; libkh2.so / libkh2.dylib
//      elsewhere) into Assets/Plugins/.
//   2. Put this file in Assets/, add Kh2Sora to a GameObject, set IsoPath to your own disc image.
//   3. Collision: every MeshCollider under CollisionRoot is sent once as kh2 triangles (walls flagged
//      solid). Sora's transform follows the library each FixedUpdate (set Fixed Timestep to 1/60).
//
// Spaces
//   kh2 is right-handed, Y up, centimetres; Unity is left-handed, Y up, metres. The conversion
//   mirrors X: unity = (-x, y, z) * 0.01. A mirror flips triangle winding, so vertices 1 and 2 are
//   swapped when triangles go to kh2, and Sora's Unity yaw is -yaw. Bone matrices convert as
//   S * M * S with S = diag(-1, 1, 1) (and the translation scaled).

// Outside Unity (examples/csharp, a plain .NET console host) define KH2_NO_UNITY: the bindings
// compile without UnityEngine, the conversion helpers and the MonoBehaviour are left out.

using System;
using System.Runtime.InteropServices;
#if !KH2_NO_UNITY
using UnityEngine;
#endif

public static class Kh2
{
    const string Lib = "kh2";

    public const uint SurfWall = 0x10;
    public const uint BtnAttack = 1, BtnJump = 2, BtnGuard = 4;

    [StructLayout(LayoutKind.Sequential)]
    public struct Surface
    {
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 9)] public float[] v;
        public uint flags;
        public byte group;
        public byte material;
        public ushort pad;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct Inputs
    {
        public float stickX, stickY, camLookX, camLookZ;
        public uint buttons;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct State
    {
        public float posX, posY, posZ;
        public float velX, velY, velZ;
        public float yaw;
        public int grounded, state, motionSlot, motionId;
        public float frame, gframe;
        public int motionFinished, blending, combo, attackActive, attackId, attackGroup;
        public int comboWindow, finisherWindow, weaponOut, groundMaterial, invulnerable, hp, maxHp, mp, maxMp;
        public float mpCharge;
        public int form, drivePoints, driveFraction;
        public float formTime, formTimeTotal;
        public int level, exp, strength, magic, defense;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct Volume
    {
        public float ax, ay, az, bx, by, bz, radius;
        public int attackId, group;
    }

    [DllImport(Lib)] public static extern int kh2_init(string isoPath);
    [DllImport(Lib)] public static extern uint kh2_version();
    [DllImport(Lib)] public static extern void kh2_terminate();
    [DllImport(Lib)] public static extern uint kh2_last_error(byte[] buf, uint len);
    [DllImport(Lib)] public static extern int kh2_surfaces_load([In] Surface[] tris, uint count);
    [DllImport(Lib)] public static extern int kh2_surfaces_load_map(string area);
    [DllImport(Lib)] public static extern void kh2_set_collision(uint group, int solid);
    [DllImport(Lib)] public static extern int kh2_create(float x, float y, float z);
    [DllImport(Lib)] public static extern void kh2_delete(int handle);
    [DllImport(Lib)] public static extern int kh2_set_position(int handle, float x, float y, float z, float yaw);
    [DllImport(Lib)] public static extern int kh2_tick(int handle, ref Inputs inputs, out State state);
    [DllImport(Lib)] public static extern int kh2_tick_field(int handle, ref Inputs inputs, out State state);
    [DllImport(Lib)] public static extern int kh2_set_target(int handle, float x, float y, float z, float radius, float halfHeight);
    [DllImport(Lib)] public static extern int kh2_clear_target(int handle);
    [DllImport(Lib)] public static extern int kh2_report_hit(int handle);
    [DllImport(Lib)] public static extern int kh2_hit_taken(int handle, int atkp, int damage, float dirX, float dirY, float dirZ);
    [DllImport(Lib)] public static extern int kh2_set_hp(int handle, int hp, int maxHp);
    [DllImport(Lib)] public static extern int kh2_revive(int handle, int automatic);
    [DllImport(Lib)] public static extern int kh2_set_high_jump(int handle, byte level);
    [DllImport(Lib)] public static extern int kh2_set_loadout(int handle, ushort[] ids, uint count);
    [DllImport(Lib)] public static extern int kh2_get_loadout_block(int handle, int[] outBlock);
    [DllImport(Lib)] public static extern int kh2_set_growth(int handle, byte highJump, byte quickRun, byte aerialDodge, byte glide);
    [DllImport(Lib)] public static extern int kh2_bone_count(int handle, int which);
    [DllImport(Lib)] public static extern int kh2_get_bones(int handle, int which, int space, [Out] float[] outMatrices, uint maxBones);
    [DllImport(Lib)] public static extern int kh2_get_attack_volumes(int handle, [Out] Volume[] outVolumes, uint max);
    [StructLayout(LayoutKind.Sequential)]
    public struct CamInput { public float stickX, stickY, zoom; public uint buttons; public float step; }

    [StructLayout(LayoutKind.Sequential)]
    public struct CamView { public float posX, posY, posZ, lookX, lookY, lookZ, fov, roll, yaw; public int mode; }

    [DllImport(Lib)] public static extern int kh2_camera_tick(int handle, ref CamInput input, out CamView view);
    [DllImport(Lib)] public static extern int kh2_camera_reset(int handle, int row);
    [DllImport(Lib)] public static extern int kh2_lock_on_pick(int handle, float[] points, uint count);
    [StructLayout(LayoutKind.Sequential)]
    public struct Event
    {
        public int kind, slot, castKind, motion, voice, charge, stage, atkp, subId;
        public float radius, height, life, speed, posX, posY, posZ, dirX, dirY, dirZ;
        public int amount, hp;
    }

    [DllImport(Lib)] public static extern int kh2_damage(int handle, int atkp, int subId, int defense, int maxHp, int weakness, int general);
    [DllImport(Lib)] public static extern int kh2_set_level(int handle, int level);
    [DllImport(Lib)] public static extern int kh2_use_item(int handle, int item);
    [DllImport(Lib)] public static extern int kh2_report_defeat(int handle, int lvpmExperience, int enmpExperience);

    [DllImport(Lib)] public static extern int kh2_cast(int handle, int slot);
    [DllImport(Lib)] public static extern int kh2_set_magic_levels(int handle, byte[] levels);
    [DllImport(Lib)] public static extern int kh2_set_mp(int handle, int mp, int maxMp);
    [DllImport(Lib)] public static extern int kh2_poll_event(int handle, out Event ev);
    [DllImport(Lib)] public static extern int kh2_drive_setup(int handle, byte points, byte maxPoints, uint ownedMask, byte[] levels);
    [DllImport(Lib)] public static extern int kh2_drive_gain(int handle, int percent);
    [DllImport(Lib)] public static extern int kh2_report_hit_atkp(int handle, int atkp);
    [DllImport(Lib)] public static extern int kh2_drive_enter(int handle, int form);
    [DllImport(Lib)] public static extern int kh2_drive_revert(int handle);
    [DllImport(Lib)] public static extern int kh2_drive_set_second_keyblade(int handle, int item);
    [DllImport(Lib)] public static extern int kh2_drive_objects(int handle, int[] outIds);
    [DllImport(Lib)] public static extern int kh2_motion_slot_count(int handle, int which);
    [DllImport(Lib)] public static extern int kh2_motion_slot(int handle, int which, int motionId, int variant);
    [StructLayout(LayoutKind.Sequential)]
    public struct MotionDesc
    {
        public float frameStart, frameEnd, frameReturn, fps;
        public int hasRootMotion;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)] public byte[] tag;
    }
    [DllImport(Lib)] public static extern int kh2_motion_info(int handle, int which, int slot, out MotionDesc info);
    [DllImport(Lib)] public static extern int kh2_get_state(int handle, out State state);
    [DllImport(Lib)] public static extern int kh2_set_abilities(int handle, ulong lo, ulong hi, byte maxGround, byte maxAir, byte maxFinishers);
    [DllImport(Lib)] public static extern int kh2_set_pose_enabled(int handle, int on);
    [DllImport(Lib)] public static extern int kh2_get_rest_bones(int handle, int which, [Out] float[] outMatrices, uint maxBones);
    [DllImport(Lib)] public static extern int kh2_bone_parents(int handle, int which, [Out] int[] outParents, uint maxBones);
    [DllImport(Lib)] public static extern float kh2_jump_height(int handle);
    [DllImport(Lib)] public static extern int kh2_drive_outgoing_damage(int handle, int atkType, int damage);
    [DllImport(Lib)] public static extern int kh2_sample_motion(int handle, int which, int slot, float frame, [Out] float[] outMatrices, uint maxBones, [Out] float[] root);
    [StructLayout(LayoutKind.Sequential)]
    public struct MemberState { public float posX, posY, posZ, yaw; public int member, motionSlot; public float frame; public int hp, maxHp, mp, maxMp, ko, level, style, strength, magic, defense; }
    [StructLayout(LayoutKind.Sequential)]
    public struct MemberEvent { public int slot, kind, id, atkp, damage, hp; [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)] public byte[] text; }
    [DllImport(Lib)] public static extern int kh2_poll_member_event(int handle, out MemberEvent ev);
    [DllImport(Lib)] public static extern int kh2_party_set_level(int handle, int slot, int level);
    [DllImport(Lib)] public static extern int kh2_party_set_vitals(int handle, int slot, int hp, int maxHp, int mp, int maxMp);
    [DllImport(Lib)] public static extern int kh2_party_set_style(int handle, int slot, int style);
    [DllImport(Lib)] public static extern int kh2_party_set_items(int handle, int slot, ushort[] ids, byte[] counts, uint count);

    [StructLayout(LayoutKind.Sequential)]
    public struct LimitState { public int command, ticksLeft, hits; }
    [StructLayout(LayoutKind.Sequential)]
    public struct LimitEvent { public int kind, a, b; public float blend; [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)] public byte[] text; }
    [DllImport(Lib)] public static extern int kh2_limit_status(int handle, [Out] ushort[] cmds, [Out] int[] status, uint max);
    [DllImport(Lib)] public static extern int kh2_limit_start(int handle, int cmd);
    [DllImport(Lib)] public static extern int kh2_limit_set_world(int handle, int world);
    [DllImport(Lib)] public static extern int kh2_limit_state(int handle, out LimitState state);
    [DllImport(Lib)] public static extern int kh2_poll_limit_event(int handle, out LimitEvent ev);

    [DllImport(Lib)] public static extern int kh2_party_add(int handle, int member);
    [DllImport(Lib)] public static extern int kh2_party_tick(int handle);
    [DllImport(Lib)] public static extern int kh2_party_state(int handle, int slot, out MemberState state);
    [DllImport(Lib)] public static extern int kh2_party_bones(int handle, int slot, int space, [Out] float[] outMatrices, uint maxBones);
    [StructLayout(LayoutKind.Sequential)]
    public struct EnemyState { public int id; public float posX, posY, posZ, yaw; public int hp, maxHp, state, motionSlot; public float frame; public int grounded; public float alpha; public int enmp; }

    [StructLayout(LayoutKind.Sequential)]
    public struct EnemyEvent
    {
        public int kind, id, atkp, damage, value, enmp, lvpmExperience, enmpExperience;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 9)] public uint[] prizes;
    }

    [DllImport(Lib)] public static extern int kh2_enemy_spawn(int handle, string kind, float x, float y, float z, float yaw);
    [DllImport(Lib)] public static extern int kh2_enemies_tick(int handle);
    [DllImport(Lib)] public static extern int kh2_enemies_prune(int handle);
    [DllImport(Lib)] public static extern int kh2_enemies_clear(int handle);
    [DllImport(Lib)] public static extern int kh2_enemies_set_mission(int handle, int on, int c0, int c1, int c2);
    [DllImport(Lib)] public static extern int kh2_area_run(int handle, string area, int program);
    [DllImport(Lib)] public static extern int kh2_area_raise_event(int handle, int ev, int arg);
    [DllImport(Lib)] public static extern int kh2_area_take_events(int handle, [Out] int[] outPairs, uint max);
    [DllImport(Lib)] public static extern int kh2_area_end(int handle);
    [StructLayout(LayoutKind.Sequential)]
    public struct Mission
    {
        public int running;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 3)] public int[] counters;
        public int clockFrames, clockDown, state, eventPlaying, stopped, endCause, result, ended, gameOver, menu;
        public uint orbs, id;
    }
    [DllImport(Lib)] public static extern int kh2_mission_state(int handle, out Mission state);
    [DllImport(Lib)] public static extern int kh2_mission_orbs(int handle, [Out] float[] outOrbs, uint max);
    [DllImport(Lib)] public static extern int kh2_mission_take_sounds(int handle, [Out] float[] outSounds, uint max);
    [StructLayout(LayoutKind.Sequential)]
    public struct TalkTarget { public float posX, posY, posZ, yaw; public int part, keepFacing, ownFrame, examine; public uint talkId; }
    [StructLayout(LayoutKind.Sequential)]
    public struct TalkRequest { public uint talkId; public int examine; public float targetYaw, soraYaw; public int targetMotion, camera; public float lookX, lookY, lookZ, dist, height; }
    [DllImport(Lib)] public static extern int kh2_talk_begin(int handle, string model, ref TalkTarget target, float[] bones, uint boneCount, out TalkRequest request);
    [DllImport(Lib)] public static extern int kh2_talk_end(int handle);
    public const int CueSound = 1, CueVoice = 2, CueEffect = 3, CueFootstep = 4, CueDust = 5, CueScriptEffect = 6, CueSoundNumber = 7;
    [StructLayout(LayoutKind.Sequential)]
    public struct Cue
    {
        public int kind, owner, ownerId;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 4)] public short[] parameters;
        public short frame, pad;
        public float posX, posY, posZ;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)] public byte[] model;
    }
    [DllImport(Lib)] public static extern int kh2_poll_cue(int handle, out Cue cue);
    public const int CueCollisionGroup = 8;
    [StructLayout(LayoutKind.Sequential)]
    public struct AreaInfo
    {
        public uint objectCount, groupCount;
        public ulong collisionMask;
        public uint cameraType;
        public ushort musicField, musicBattle;
        public int hasPlayer;
        public float playerX, playerY, playerZ, playerYaw;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct AreaObject
    {
        public uint obj;
        public int objectType;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)] public byte[] model;
        public int script, program;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)] public byte[] group;
        public float posX, posY, posZ, yaw;
        public uint talkId, partCommand;
        public int spawnKind;
        public uint serial;
    }
    [DllImport(Lib)] public static extern int kh2_area_setup(string area, int map, int btl, int evt, int entrance, out AreaInfo info);
    [DllImport(Lib)] public static extern int kh2_area_get_object(int index, out AreaObject obj);
    [StructLayout(LayoutKind.Sequential)]
    public struct MeshInfo { public uint vertexCount, indexCount, submeshCount, pad; }
    [StructLayout(LayoutKind.Sequential)]
    public struct Submesh { public uint firstIndex, indexCount; public int texture, transparent; }
    [DllImport(Lib)] public static extern int kh2_mesh_prepare(int handle, int owner, int part, int space, out MeshInfo info);
    [DllImport(Lib)] public static extern int kh2_mesh_get([Out] float[] positions, [Out] float[] normals, [Out] float[] uvs, [Out] float[] colors, uint maxVertices, [Out] uint[] indices, uint maxIndices, [Out] Submesh[] subs, uint maxSubs);
    [DllImport(Lib)] public static extern int kh2_texture_info(int id, out uint width, out uint height);
    [DllImport(Lib)] public static extern int kh2_texture_get(int id, [Out] byte[] rgba, uint cap);
    public const uint HudUp = 1, HudDown = 2, HudConfirm = 4, HudBack = 8, HudReaction = 16;
    [StructLayout(LayoutKind.Sequential)]
    public struct HudSprite
    {
        public int texture, blend, nearest, text;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)] public float[] pos;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)] public float[] uv;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)] public byte[] color;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct HudEvent { public int kind, value; }
    [DllImport(Lib)] public static extern int kh2_hud_load(int handle, string world);
    [DllImport(Lib)] public static extern int kh2_hud_input(int handle, uint buttons);
    [DllImport(Lib)] public static extern int kh2_hud_set_items(int handle, ushort[] ids, byte[] counts, uint count);
    [DllImport(Lib)] public static extern int kh2_hud_tick(int handle);
    [DllImport(Lib)] public static extern int kh2_hud_draw_list(int handle, [Out] HudSprite[] sprites, uint max);
    [DllImport(Lib)] public static extern int kh2_hud_poll_event(int handle, out HudEvent ev);
    [DllImport(Lib)] public static extern int kh2_hud_texture_info(int handle, int id, out uint width, out uint height);
    [DllImport(Lib)] public static extern int kh2_hud_texture_get(int handle, int id, [Out] byte[] rgba, uint cap);
    [StructLayout(LayoutKind.Sequential)]
    public struct ObjectTransform { public float posX, posY, posZ, rotX, rotY, rotZ; }
    [DllImport(Lib)] public static extern int kh2_surface_object_create([In] Surface[] tris, uint count, ref ObjectTransform transform);
    [DllImport(Lib)] public static extern int kh2_surface_object_move(int id, ref ObjectTransform transform);
    [DllImport(Lib)] public static extern int kh2_surface_object_delete(int id);
    public const int CamModeFollow = 0, CamModeLook = 1, CamModeLockOn = 2, CamModeTalk = 4, CamModeOverhead = 5, CamModeBehind = 8;
    [DllImport(Lib)] public static extern int kh2_area_spawn(int handle, string area, int program, int withDisabled);
    [DllImport(Lib)] public static extern int kh2_enemy_count(int handle);
    [DllImport(Lib)] public static extern int kh2_enemy_state(int handle, int index, out EnemyState state);
    [DllImport(Lib)] public static extern int kh2_enemy_bones(int handle, int index, int space, [Out] float[] outMatrices, uint maxBones);
    [DllImport(Lib)] public static extern int kh2_poll_enemy_event(int handle, out EnemyEvent ev);

    [StructLayout(LayoutKind.Sequential)]
    public struct ReactionOffer { public int cmd, from, status; public uint message; }
    [DllImport(Lib)] public static extern int kh2_reaction_offer(int handle, out ReactionOffer offer);
    [DllImport(Lib)] public static extern int kh2_reaction_accept(int handle);
    [DllImport(Lib)] public static extern int kh2_reaction_force_offer(int handle, int cmd);

    [DllImport(Lib)] public static extern int kh2_set_weapon_set(int handle, int set);
    [DllImport(Lib)] public static extern int kh2_give_bonus(int handle, int id);
    [StructLayout(LayoutKind.Sequential)]
    public struct ItemInfo { public ushort id; public byte kind, ap; public ushort command, name, description, amount, shopBuy, shopSell; public byte attack, magic, defense; [MarshalAs(UnmanagedType.ByValArray, SizeConst = 7)] public byte[] resist; public ushort ability; }
    [DllImport(Lib)] public static extern int kh2_item_info(int id, out ItemInfo info);
    [DllImport(Lib)] public static extern int kh2_set_equipment(int handle, ushort[] ids, uint count);
    [DllImport(Lib)] public static extern int kh2_party_set_equipment(int handle, int slot, ushort[] ids, uint count);

    public const int SummonDismiss = 0x13c;
    [StructLayout(LayoutKind.Sequential)]
    public struct SummonState { public int state, index, arriving; public float timer, timerTotal; public int segments, level, exp, obj; public float posX, posY, posZ, yaw; public int motionSlot, companionsAway; }
    [StructLayout(LayoutKind.Sequential)]
    public struct SummonEvent { public int kind, value; public float timer; [MarshalAs(UnmanagedType.ByValArray, SizeConst = 32)] public byte[] text; }
    [DllImport(Lib)] public static extern int kh2_summon_set_owned(int handle, ushort[] items, uint count);
    [DllImport(Lib)] public static extern int kh2_summon_status(int handle, [Out] ushort[] cmds, [Out] int[] status, uint max);
    [DllImport(Lib)] public static extern int kh2_summon(int handle, int cmd);
    [DllImport(Lib)] public static extern int kh2_summon_set_arrival_event(int handle, int on);
    [DllImport(Lib)] public static extern int kh2_summon_state(int handle, out SummonState state);
    [DllImport(Lib)] public static extern int kh2_summon_bones(int handle, int space, [Out] float[] outMatrices, uint maxBones);
    [DllImport(Lib)] public static extern int kh2_poll_summon_event(int handle, out SummonEvent ev);
    [DllImport(Lib)] public static extern int kh2_form_abilities(int handle, int form, [Out] ushort[] outIds, uint max);
    [DllImport(Lib)] public static extern int kh2_granted_abilities(int handle, [Out] ushort[] outIds, uint max);
    [DllImport(Lib)] public static extern int kh2_state_save(int handle, [Out] byte[] outBytes, uint cap);
    [DllImport(Lib)] public static extern int kh2_state_load(int handle, byte[] bytes, uint len);
    [DllImport(Lib)] public static extern int kh2_motion_tag(int handle, byte[] buf, uint len);

    public static string LastError()
    {
        var b = new byte[512];
        kh2_last_error(b, (uint)b.Length);
        return System.Text.Encoding.UTF8.GetString(b).TrimEnd('\0');
    }

#if !KH2_NO_UNITY
    // Unity metres <-> kh2 centimetres, X mirrored.
    public static Vector3 ToUnity(float x, float y, float z) => new Vector3(-x, y, z) * 0.01f;
    public static Vector3 ToKh2(Vector3 u) => new Vector3(-u.x, u.y, u.z) * 100f;

    public static Matrix4x4 ToUnity(float[] m, int at)
    {
        var k = new Matrix4x4();
        for (int c = 0; c < 4; c++)
            for (int r = 0; r < 4; r++)
                k[r, c] = m[at + 4 * c + r];
        var s = Matrix4x4.Scale(new Vector3(-1, 1, 1));
        var u = s * k * s;
        u.SetColumn(3, new Vector4(u.m03 * 0.01f, u.m13 * 0.01f, u.m23 * 0.01f, 1));
        return u;
    }
#endif
}

#if !KH2_NO_UNITY
public class Kh2Sora : MonoBehaviour
{
    public string IsoPath = "";
    public Transform CollisionRoot;
    public Camera Cam;
    int handle = -1;
    Kh2.State state;
    public int Handle => handle;

    void Start()
    {
        if (Kh2.kh2_init(IsoPath) != 0) { Debug.LogError("kh2_init: " + Kh2.LastError()); enabled = false; return; }
        var tris = new System.Collections.Generic.List<Kh2.Surface>();
        foreach (var mc in CollisionRoot.GetComponentsInChildren<MeshCollider>())
        {
            var mesh = mc.sharedMesh; var tf = mc.transform; var idx = mesh.triangles; var vs = mesh.vertices;
            for (int i = 0; i < idx.Length; i += 3)
            {
                // Swap 1 and 2: the X mirror flips the winding.
                Vector3 a = Kh2.ToKh2(tf.TransformPoint(vs[idx[i]])), b = Kh2.ToKh2(tf.TransformPoint(vs[idx[i + 2]])), c = Kh2.ToKh2(tf.TransformPoint(vs[idx[i + 1]]));
                var n = Vector3.Cross(b - a, c - a).normalized;
                tris.Add(new Kh2.Surface { v = new[] { a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z }, flags = Mathf.Abs(n.y) < 0.5f ? Kh2.SurfWall : 1u });
            }
        }
        Kh2.kh2_surfaces_load(tris.ToArray(), (uint)tris.Count);
        var p = Kh2.ToKh2(transform.position);
        handle = Kh2.kh2_create(p.x, p.y, p.z);
    }

    void FixedUpdate()
    {
        if (handle < 0) return;
        var look = Cam ? Cam.transform.forward : Vector3.forward;
        var input = new Kh2.Inputs
        {
            stickX = Input.GetAxis("Horizontal"),
            stickY = Input.GetAxis("Vertical"),
            camLookX = -look.x,
            camLookZ = look.z,
            buttons = (Input.GetKey(KeyCode.J) ? Kh2.BtnAttack : 0) | (Input.GetKey(KeyCode.Space) ? Kh2.BtnJump : 0) | (Input.GetKey(KeyCode.K) ? Kh2.BtnGuard : 0),
        };
        Kh2.kh2_tick(handle, ref input, out state);
        transform.position = Kh2.ToUnity(state.posX, state.posY, state.posZ);
        transform.rotation = Quaternion.Euler(0, -state.yaw * Mathf.Rad2Deg, 0);
        // Test the live attack capsules against your own targets here and call kh2_report_hit on contact.
    }

    void OnDestroy()
    {
        if (handle >= 0) Kh2.kh2_delete(handle);
        Kh2.kh2_terminate();
    }
}
#endif
