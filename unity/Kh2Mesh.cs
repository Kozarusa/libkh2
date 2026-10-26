// Draws a libkh2 body with Unity (round 10): kh2_mesh_prepare + kh2_mesh_get every frame into a
// Mesh, one Texture2D per texture id. Written against the Unity API; not built in this repository
// (UnityEngine is not available here). See docs/MESHES.md.
//
// Spaces: kh2 is right-handed, Y up, centimetres; Unity left-handed, metres: x is mirrored and
// scaled by 0.01, triangle winding flipped (as in Kh2.cs).

#if !KH2_NO_UNITY
using System.Collections.Generic;
using UnityEngine;

[RequireComponent(typeof(MeshFilter), typeof(MeshRenderer))]
public class Kh2Mesh : MonoBehaviour
{
    public Kh2Sora Sora;          // the handle's owner (Kh2.cs)
    public int Owner = 0;         // 0 Sora, 1 member, 2 enemy, 3 summon
    public int Part = 0;          // Sora: 0 body, 1 keyblade; member slot; enemy index
    public Shader Shader;         // an unlit, vertex-colour, alpha-cutout or blended shader

    Mesh mesh;
    float[] pos, nrm, uv, col;
    uint[] idx;
    Kh2.Submesh[] subs;
    readonly Dictionary<int, Material> materials = new Dictionary<int, Material>();

    void Start()
    {
        mesh = new Mesh { indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
        mesh.MarkDynamic();
        GetComponent<MeshFilter>().sharedMesh = mesh;
    }

    void LateUpdate()
    {
        if (Sora == null || Sora.Handle < 0) return;
        if (Kh2.kh2_mesh_prepare(Sora.Handle, Owner, Part, 1, out var info) != 0) return;
        int nv = (int)info.vertexCount, ni = (int)info.indexCount, ns = (int)info.submeshCount;
        if (pos == null || pos.Length < 3 * nv) { pos = new float[3 * nv]; nrm = new float[3 * nv]; uv = new float[2 * nv]; col = new float[4 * nv]; }
        if (idx == null || idx.Length < ni) idx = new uint[ni];
        if (subs == null || subs.Length < ns) subs = new Kh2.Submesh[ns];
        Kh2.kh2_mesh_get(pos, nrm, uv, col, (uint)nv, idx, (uint)ni, subs, (uint)ns);

        var v = new Vector3[nv]; var n = new Vector3[nv]; var t = new Vector2[nv]; var c = new Color[nv];
        for (int i = 0; i < nv; i++)
        {
            v[i] = new Vector3(-pos[3 * i], pos[3 * i + 1], pos[3 * i + 2]) * 0.01f;
            n[i] = new Vector3(-nrm[3 * i], nrm[3 * i + 1], nrm[3 * i + 2]);
            t[i] = new Vector2(uv[2 * i], 1f - uv[2 * i + 1]);
            c[i] = new Color(col[4 * i], col[4 * i + 1], col[4 * i + 2], col[4 * i + 3]);
        }
        mesh.Clear();
        mesh.vertices = v; mesh.normals = n; mesh.uv = t; mesh.colors = c;
        mesh.subMeshCount = ns;
        var mats = new Material[ns];
        for (int k = 0; k < ns; k++)
        {
            var tri = new int[subs[k].indexCount];
            for (int i = 0; i < tri.Length; i += 3)
            {
                tri[i] = (int)idx[subs[k].firstIndex + i];
                tri[i + 1] = (int)idx[subs[k].firstIndex + i + 2];   // mirrored: flip the winding
                tri[i + 2] = (int)idx[subs[k].firstIndex + i + 1];
            }
            mesh.SetTriangles(tri, k);
            mats[k] = MaterialFor(subs[k].texture, subs[k].transparent != 0);
        }
        GetComponent<MeshRenderer>().sharedMaterials = mats;
        transform.SetPositionAndRotation(Vector3.zero, Quaternion.identity);   // the vertices are in world space
    }

    Material MaterialFor(int id, bool transparent)
    {
        int key = id * 2 + (transparent ? 1 : 0);
        if (materials.TryGetValue(key, out var m)) return m;
        m = new Material(Shader);
        if (id >= 0 && Kh2.kh2_texture_info(id, out var w, out var h) == 0)
        {
            var px = new byte[w * h * 4];
            Kh2.kh2_texture_get(id, px, (uint)px.Length);
            // kh2 rows run from the top; Unity's from the bottom.
            var flipped = new byte[px.Length];
            int row = (int)w * 4;
            for (int y = 0; y < h; y++) System.Buffer.BlockCopy(px, y * row, flipped, ((int)h - 1 - y) * row, row);
            var tex = new Texture2D((int)w, (int)h, TextureFormat.RGBA32, false);
            tex.LoadRawTextureData(flipped);
            tex.Apply();
            m.mainTexture = tex;
        }
        if (transparent) m.renderQueue = 3000;
        materials[key] = m;
        return m;
    }
}
#endif
