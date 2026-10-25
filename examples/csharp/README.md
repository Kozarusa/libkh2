# C# console sample

`Program.cs` drives libkh2 from .NET through the Unity bindings `unity/Kh2.cs` (P/Invoke), compiled
with `KH2_NO_UNITY` so no UnityEngine is needed: the version check, Sora on a floor (run, jump,
attack), then Seifer's Struggle match in tt04 (`kh2_area_run` program 184) with the mission
manager's orbs, clock and result.

```
cargo build --release -p libkh2
dotnet run -c Release --project examples/csharp -- "<path to the USA disc image>"
```

(or set `KH2_ISO` and pass nothing). Needs a .NET 9 SDK; the build goes to `target/csharp/` and
copies `target/release/kh2.dll` next to the program. A run on the USA disc prints the floor run and
then, about every 10 s of game time, the orbs (Sora's climb from 100, Seifer's fall to 0), the clock
counting down from 60 s and, at its end, `end cause 3, result won`.

`python scripts/check_bindings.py` diffs the DLL's exports against `kh2.h` and `Kh2.cs`.
