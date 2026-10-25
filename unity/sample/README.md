# libkh2 Unity sample

A folder to drop into a Unity project (2021.3 or later): it is not built in this repository.

1. Build the library: `cargo build --release -p libkh2` (or run `scripts/package.ps1` and take
   `dist/libkh2/bin/kh2.dll`). Copy `kh2.dll` (Windows), `libkh2.so` or `libkh2.dylib` into
   `Assets/Plugins/` of your project.
2. Copy `unity/Kh2.cs` and `unity/sample/Assets/Scripts/Kh2Battle.cs` into `Assets/Scripts/`.
3. Make a scene with a floor that has a MeshCollider under an empty "Collision" object, a camera,
   and a GameObject with `Kh2Sora` (IsoPath = your own disc image, CollisionRoot = "Collision",
   Cam = the camera) and `Kh2Battle`.
4. Set Project Settings > Time > Fixed Timestep to 0.0166667 (60 Hz, the game's logic rate).
5. Play: WASD / arrows move, Space jumps, J attacks, K guards; E spawns enemies, Q locks on, T
   accepts a reaction command. Donald and Goofy follow and fight; bones are drawn as gizmos.

Spaces: kh2 is right-handed in centimetres, Unity left-handed in metres; `Kh2.ToUnity` / `Kh2.ToKh2`
mirror X and scale (see the notes at the top of `Kh2.cs` and `docs/UNITY.md`).

No game data is in this folder; the library reads everything from the disc image at run time.
