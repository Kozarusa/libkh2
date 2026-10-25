// libkh2 Unity sample: Donald, Goofy and enemies beside the Kh2Sora driver of unity/Kh2.cs.
//
// Add Kh2Sora and this component to the same GameObject (Kh2Sora inits the library and creates Sora).
// Press E in play mode to spawn a Shadow, a Soldier and a Dusk in front of Sora; Q locks on to the
// nearest; T accepts a reaction command. The members and enemies are drawn as bone gizmos (Scene and
// Game view with Gizmos on); a real host skins its own models with the same matrices.

using UnityEngine;

[RequireComponent(typeof(Kh2Sora))]
public class Kh2Battle : MonoBehaviour
{
    public bool Party = true;
    Kh2Sora sora;
    readonly float[] bones = new float[16 * 256];
    bool locked;

    void Start()
    {
        sora = GetComponent<Kh2Sora>();
    }

    void FixedUpdate()
    {
        int h = sora.Handle;
        if (h < 0) return;
        if (Party && Kh2.kh2_party_tick(h) < 0)
        {
            Kh2.kh2_party_add(h, 0);
            Kh2.kh2_party_add(h, 1);
        }
        if (Input.GetKeyDown(KeyCode.E))
        {
            var f = transform.forward;
            var at = Kh2.ToKh2(transform.position + f * 5f);
            float yaw = Mathf.Atan2(f.x, -f.z);
            foreach (var kind in new[] { "M_EX020", "M_EX010", "M_EX990" })
                Kh2.kh2_enemy_spawn(h, kind, at.x, at.y, at.z, yaw);
        }
        Kh2.kh2_enemies_tick(h);
        while (Kh2.kh2_poll_enemy_event(h, out var ev) == 1)
            if (ev.kind == 3) Debug.Log($"enemy {ev.id} defeated, {ev.value} EXP");
        if (Input.GetKeyDown(KeyCode.Q)) locked = !locked;
        if (locked && Nearest(h, out var p)) Kh2.kh2_set_target(h, p.posX, p.posY + 80f, p.posZ, 40f, 80f);
        else Kh2.kh2_clear_target(h);
        if (Input.GetKeyDown(KeyCode.T)) Kh2.kh2_reaction_accept(h);
    }

    bool Nearest(int h, out Kh2.EnemyState best)
    {
        best = default;
        float d = float.MaxValue;
        var me = Kh2.ToKh2(transform.position);
        for (int i = 0; i < Kh2.kh2_enemy_count(h); i++)
        {
            Kh2.kh2_enemy_state(h, i, out var e);
            if (e.hp <= 0) continue;
            float dd = (new Vector3(e.posX, e.posY, e.posZ) - me).sqrMagnitude;
            if (dd < d) { d = dd; best = e; }
        }
        return d < float.MaxValue;
    }

    void DrawBones(int n, Color c)
    {
        Gizmos.color = c;
        for (int i = 1; i < n; i++)
            Gizmos.DrawLine(Kh2.ToUnity(bones, 16 * (i - 1)).GetColumn(3), Kh2.ToUnity(bones, 16 * i).GetColumn(3));
    }

    void OnDrawGizmos()
    {
        if (sora == null || sora.Handle < 0) return;
        int h = sora.Handle;
        for (int s = 0; s < 2; s++)
        {
            int n = Kh2.kh2_party_bones(h, s, 1, bones, 256);
            if (n > 0) DrawBones(n, s == 0 ? Color.cyan : Color.green);
        }
        for (int i = 0; i < Kh2.kh2_enemy_count(h); i++)
        {
            int n = Kh2.kh2_enemy_bones(h, i, 1, bones, 256);
            if (n > 0) DrawBones(n, Color.red);
        }
    }
}
