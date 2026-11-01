# SLICE-0 — Baseline build

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: PASSED

## Source
- HarbourMasters/SpaghettiKart `ce0f935` (2026-10-22), cloned with `--recursive`. Same commit RECON-003 was read from.
- Submodules: libultraship `60c9bb7`, torch `2d474dd`, fast64 `d89593c`, doxygen-awesome-css `9760c30`.
- ROM: `baserom.us.z64`, SHA-1 `579C48E211AE952530FFC8738709F078D5DD215E` [VERIFIED].

## Toolchain
- Visual Studio Build Tools 2026 with MSVC v143 14.44.35207 added (v145 14.51 also installed). Windows SDK 10.0.26100.
- CMake 4.3.1 bundled with the Build Tools (not on PATH). Git 2.56. Python 3.14.7.

BUILDING.md assumes VS 2022, so the generator differs from upstream docs:

```powershell
$cm = "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
& $cm -S . -B "build/x64" -G "Visual Studio 18 2026" -T v143 -A x64
& $cm --build .\build\x64 --target ExtractAssets
& $cm --build .\build\x64
```

## Results [VERIFIED: build output]
| Step | Result | Time |
|---|---|---|
| Configure (first run, includes vcpkg deps) | OK | 399 s |
| ExtractAssets → `mk64.o2r` 26.7 MB, `spaghetti.o2r` 2.7 MB | OK | 286 s |
| Compile (Debug) → `build\x64\Debug\Spaghettify.exe` | OK, 0 errors | 294 s |

- 349 compiler warnings, all conversion warnings: C4244 ×203, C4267 ×120, C4554 ×4, C4838 ×4, C4005 ×3, C4805 ×2. Typical for decomp code.
- Configure warnings: CMake dev/deprecation notices only (ASM policy, `_deps/prism-src`).

## In-game test [VERIFIED: user playtest]
- Launch from `build\x64` as the working directory so `mk64.o2r` is found: `.\Debug\Spaghettify.exe`.
- Renderer: DirectX 11.
- User drove a race: "ran extremely well". No crashes or glitches reported. The settings menu is extensive.

## Nothing broken found.
