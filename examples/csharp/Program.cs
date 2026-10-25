// libkh2 from C#: the version check, Sora on a floor (run, jump, attack), then Seifer's Struggle
// match in tt04 (kh2_area_run program 184) with the mission manager's counters, clock and result.
//
//   dotnet run -c Release --project examples/csharp -- <path to the USA disc image>   (or set KH2_ISO)

using System;

static class Program
{
    const uint HeaderVersion = (0 << 16) | (8 << 8) | 0;

    static Kh2.Surface Tri(float[] a, float[] b, float[] c, uint flags) =>
        new Kh2.Surface { v = new[] { a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2] }, flags = flags };

    static int Main(string[] args)
    {
        uint v = Kh2.kh2_version();
        Console.WriteLine($"libkh2 {v >> 16}.{(v >> 8) & 0xff}.{v & 0xff}");
        if ((v >> 8) != (HeaderVersion >> 8))
        {
            Console.Error.WriteLine($"Kh2.cs is for {HeaderVersion:x}, the library is {v:x}");
            return 1;
        }
        string iso = args.Length > 0 ? args[0] : null;
        if (Kh2.kh2_init(iso) != 0)
        {
            Console.Error.WriteLine("kh2_init: " + Kh2.LastError());
            return 2;
        }

        // A floor 4000 x 4000 at y = 0.
        float s = 2000f;
        float[] f0 = { -s, 0, -s }, f1 = { -s, 0, s }, f2 = { s, 0, s }, f3 = { s, 0, -s };
        var tris = new[] { Tri(f0, f1, f2, 1), Tri(f0, f2, f3, 1) };
        Console.WriteLine($"surfaces: {Kh2.kh2_surfaces_load(tris, (uint)tris.Length)}");
        int h = Kh2.kh2_create(0, 0, 0);
        var st = new Kh2.State();
        for (int t = 0; t < 180; t++)
        {
            var input = new Kh2.Inputs { camLookX = 0, camLookZ = 1 };
            if (t < 60) input.stickY = 1;
            if (t >= 70 && t < 100) input.buttons |= Kh2.BtnJump;
            if (t >= 120 && t % 6 == 0) input.buttons |= Kh2.BtnAttack;
            Kh2.kh2_tick(h, ref input, out st);
            if (st.attackActive != 0) Kh2.kh2_report_hit(h);
            if (t % 30 == 0)
                Console.WriteLine($"t={t,3} pos=({st.posX,7:F1} {st.posY,6:F1} {st.posZ,7:F1}) state={st.state} motion={st.motionId} combo={st.combo}");
        }

        // Seifer's Struggle match: tt04's collision, the area run of program 184 (TT04_MS02).
        if (Kh2.kh2_surfaces_load_map("tt04") < 0) { Console.Error.WriteLine("tt04: " + Kh2.LastError()); return 3; }
        int made = Kh2.kh2_area_run(h, "tt04", 184);
        Console.WriteLine($"area run tt04 184: {made} created, {Kh2.kh2_enemy_count(h)} enemies");
        if (Kh2.kh2_enemy_count(h) > 0 && Kh2.kh2_enemy_state(h, 0, out var e0) == 0)
            Kh2.kh2_set_position(h, e0.posX, e0.posY, e0.posZ - 300, 0);
        var m = new Kh2.Mission();
        for (int t = 0; t < 60 * 70; t++)
        {
            var input = new Kh2.Inputs { camLookZ = 1 };
            if (Kh2.kh2_enemy_count(h) > 0 && Kh2.kh2_enemy_state(h, 0, out var e) == 0)
            {
                float dx = e.posX - st.posX, dz = e.posZ - st.posZ, d = MathF.Sqrt(dx * dx + dz * dz);
                if (d > 150) { input.stickX = dx / d; input.stickY = dz / d; }
                Kh2.kh2_set_target(h, e.posX, e.posY + 80, e.posZ, 40, 80);
            }
            if (t % 4 == 0) input.buttons |= Kh2.BtnAttack;
            Kh2.kh2_tick(h, ref input, out st);
            Kh2.kh2_enemies_tick(h);
            if (Kh2.kh2_mission_state(h, out m) == 1 && t % 600 == 0)
                Console.WriteLine($"t={t / 60,3}s orbs Sora {m.counters[0]} Seifer {m.counters[1]}, clock {m.clockFrames / 60}s, pool {m.orbs}, state {m.state}");
            if (m.result != 0 || m.ended != 0) break;
        }
        Console.WriteLine($"end cause {m.endCause}, result {(m.result == 1 ? "won" : m.result == 2 ? "lost" : "none")}, orbs Sora {m.counters[0]} Seifer {m.counters[1]}");
        Kh2.kh2_delete(h);
        Kh2.kh2_terminate();
        return 0;
    }
}
