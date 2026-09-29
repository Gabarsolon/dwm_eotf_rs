# About

`dwm_eotf_rs` (fork with Translucent Border Fix & Linear Nits Scaling) fixes the washed-out look in Windows UI, applications, and SDR games when HDR is enabled by replacing DWM's piecewise sRGB transfer function with a [proper gamma curve](https://github.com/dylanraga/win11hdr-srgb-to-gamma2.2-icm).

It does this by reading the memory of the loaded `dwmcore.dll` module, patching the shaders responsible for SDR-to-HDR conversion and translucent alpha correction, and writing them back.

### Fork Improvements & Bug Fixes:
1. **Translucent Border Fix (`AlphaCorrectSDR` Neutralization):**
   - In upstream `dwm_eotf_rs`, increasing brightness beyond 1.0 caused 1px borders of UI elements on the Taskbar, Settings window, and Mica/Acrylic cards to completely vanish.
   - **Root Cause:** DWM includes 15 internal shaders (`AlphaCorrectSDR`, `AlphaCorrectExtendedSDR`, `BoostSDRLuminance`) that apply an empirical polynomial delta to translucent pixels ($0 < \alpha < 1$). Microsoft designed this assuming luminance $\le 1.0$. Under brightness boost ($L > 1.0$), the delta becomes strongly negative and `mad_sat` clamps the border alpha and RGB to `0.0`.
   - **Fix:** Automatically neutralizes these polynomial coefficients in all 15 matching shaders. Translucent borders and outlines remain crisp, visible, and intact at any brightness.
2. **Corrected Linear Luminance (Nits) Scaling:**
   - Upstream used `scale = brightness.powf(0.5 / gamma)`, which accidentally applied $\sqrt{\text{brightness}}$ instead of linear physical luminance.
   - Fixed to `scale = brightness.powf(1.0 / gamma)` so that `--brightness 2.0` actually delivers 2.0x linear nits.
3. **Direct `--nits <NITS>` Option:**
   - Automatically queries your display's current Windows SDR white level from the registry (e.g., 480 nits at 100% SDR slider) and calculates the exact multiplier needed to hit your target nits (e.g. `--nits 1000`).
4. **Complete Forward EOTF Whitelist:**
   - Expanded the whitelist to include all 6 forward SDR-to-scRGB conversion shaders across all DWM feature levels and shader bundles (SM 4.0 and Level 9).

**You do not need to disable/revert the patch (or restart DWM) when playing HDR games or videos. It only affects DWM composed SDR content!**

# Usage

## Help Output
```
Patches DWM's shaders to use proper EOTF (gamma)

Usage: dwm_eotf_rs.exe [OPTIONS] [GAMMA] [COMMAND]

Commands:
  dump        Dumps DWM's original shaders as DXBC
  restore     Restores original sRGB EOTF and enables Multi-Plane Overlay
  schedule    Creates a task ('dwm_eotf_rs') that runs the app on user logon
  unschedule  Removes the startup task from Task Scheduler
  help        Print this message or the help of the given subcommand(s)

Arguments:
  [GAMMA]  Exponent to use during EOTF patching [default: 2.2]

Options:
      --brightness <BRIGHTNESS>  Optional brightness multiplier (factor for nits, e.g. 1.56, or 2.083 for 1000 nits) [default: 1]
      --nits <NITS>              Target SDR brightness in nits (e.g. 1000). Automatically calculates multiplier based on display white level
      --no-fix-borders           Disable border & Mica alpha correction fix (borders remain visible by default)
  -c, --compatibility-mode       Patch DWM and exit (disables tray mode)
  -d, --disable-mpo              Disable Multi-Plane Overlay (prevents windows from bypassing DWM)
  -i, --ignore-whitelist         Patch every shader that contains sRGB EOTF patterns
  -s, --skip-patching            Prevent automatic patching on app start (tray mode only)
  -w, --wait-time <WAIT_TIME>    Delay (in seconds) before patching on start (tray mode only) [default: 5]
  -h, --help                     Print help
  -V, --version                  Print version
```

## Quick Start Examples

- **Target 1000 Nits directly with 2.2 Gamma (Tray Mode):**
  ```powershell
  .\dwm_eotf_rs.exe --nits 1000 2.2
  ```

- **Target 1000 Nits and exit (Compatibility Mode):**
  ```powershell
  .\dwm_eotf_rs.exe -c --nits 1000 2.2
  ```

- **Set specific brightness factor (e.g. 1.56x) without losing borders:**
  ```powershell
  .\dwm_eotf_rs.exe --brightness 1.56 2.2
  ```

## Tray Mode
By default, the app runs in the system tray, where you can toggle the patch as needed as well as select a gamma value (2.0/2.2/2.4/[GAMMA]).

## Compatibility Mode
When supplied with the `-c` flag, `dwm_eotf_rs` works like a simple console app — it patches DWM and exits.

## Startup
The app can register itself to run automatically when the user logs in, using the Windows Task Scheduler (task named `dwm_eotf_rs`).
The `-c schedule` option schedules `dwm_eotf_rs` to autostart in Compatibility Mode.

# Acknowledgements
- Many thanks to [SERGEYDJUM](https://github.com/SERGEYDJUM) for `dwm_eotf_rs`.
- Many thanks to [ledoge](https://github.com/ledoge) for the original C implementation.
