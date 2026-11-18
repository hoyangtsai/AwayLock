# awaylock

English | [繁體中文](README.zh-tw.md)

A menu bar app that automatically locks your computer when you walk away. Currently supports macOS, with an architecture prepared for Linux.

awaylock scans Bluetooth Low Energy (BLE) signals from devices you carry. While your computer is unlocked,
if any device on your allowlist moves beyond the configured distance or stops being detected, awaylock automatically locks the screen.

> **Status: In development.** The core logic is written in Rust, with a native Swift interface for macOS. The app builds and launches,
> but Bluetooth scanning and actual screen locking have not yet been fully verified on real hardware. See “Limitations” below and [HANDOFF.md](HANDOFF.md) for known issues.

## Features
- Runs exclusively in the menu bar, without a Dock icon. Click the icon to open a menu with **Settings** and **Quit**.
- Settings window with a native macOS appearance and toolbar-style tabs. The app stays in the menu bar when the window is closed:
  - **General**: Launch at login and departure distance (1–5 meters, in 0.5-meter increments).
  - **Devices**: Scan nearby Bluetooth devices and manage the allowlist. Only devices whose names identify them as phones or watches are listed.
  - **About**
- Distances are displayed in meters, so there is no need to interpret signal strength values.
- Development builds include a **Debug** tab showing detected devices, live signals, and decision details. This code is entirely excluded from release builds.

## Locking Rules
1. Each time the computer is unlocked, a new monitoring session begins.
2. During the session, an allowlisted device is added to monitoring once it is detected within the configured distance.
3. If any monitored device moves beyond the configured distance or stops being detected (no advertisement received for more than 15 seconds),
   and remains in that state for 3 consecutive checks (performed every 5 seconds), the screen is locked.
4. Devices that have never been detected during the session do not trigger a lock.
5. After a lock is triggered, the monitored device list is cleared. Monitoring restarts only after the computer has actually locked and is then unlocked again.

## Requirements
- Rust (stable) and Xcode (Swift 5.9 or later).
- macOS 13 or later; grant Bluetooth permission on first launch.
- Set “System Settings → Lock Screen → Require password” to “**Immediately**.” The app puts the display to sleep (`pmset displaysleepnow`);
  without this setting, waking the display will not require a password.

## Build and Run
```bash
scripts/bundle-macos.sh release   # Release build → dist/awaylock.app
scripts/bundle-macos.sh dev       # Development build → dist/awaylock Dev.app (includes the Debug tab)
```

```bash
open dist/awaylock.app
```

- The script first compiles the Rust core into a dynamic library, then builds and links the Swift interface, and finally assembles the `.app` bundle.
- Run the bundled `.app`: macOS requires the Bluetooth permission description in `Info.plist`. Without it, the process will abort.
- Development builds use a separate bundle ID and settings directory, so they do not interfere with release builds.
- Debug features are gated by `#[cfg(debug_assertions)]` in Rust and `#if DEBUG` in Swift,
  and are compiled only into development builds.
- Both builds use ad-hoc signing. You may need to grant Bluetooth permission again after each rebuild.
- For reliable launch-at-login behavior, place the app in `/Applications`.

Run the core logic tests:

```bash
cargo test --workspace
```

## Installation (Homebrew)
```bash
brew install --cask hoyangtsai/app/awaylock
```
Only Apple Silicon (arm64) builds are currently available. The app is not notarized by Apple. Homebrew automatically removes the quarantine attribute during installation, allowing the app to open directly.

## Publishing a New Version (Maintainers)
Releases are built by GitHub Actions when a version tag is pushed. Tag the commit you want to release (on `main`):

```bash
git tag vX.Y.Z && git push origin vX.Y.Z
```

The tag must look like `v0.2.0` (`vMAJOR.MINOR.PATCH`) and be higher than the latest release. The version number comes from the tag name; pushing to `main` does not publish anything.

The `Release` workflow (`.github/workflows/release.yml`) then:
1. Checks that the `TAP_TOKEN` secret can push to the tap repository.
2. Runs `cargo test --workspace`.
3. Runs `scripts/package-macos.sh` to build `awaylock-<version>-arm64.zip` and the cask file.
4. Creates the GitHub release for the tag and uploads the ZIP.
5. Commits the updated cask to `Casks/awaylock.rb` in the `hoyangtsai/homebrew-app` tap.

`TAP_TOKEN` is a repository secret: a fine-grained personal access token with **Contents: Read and write** on `hoyangtsai/homebrew-app`. See [HANDOFF.md](HANDOFF.md) for details and troubleshooting.

## Usage
1. Open the app and go to the **Devices** tab.
2. Bring your phone, watch, or Bluetooth accessory close to your Mac. Find the closest device under “Nearby Devices” and click “Add.”
3. Adjust the departure distance in **General**.
4. While the computer is unlocked, walk away with the device. The screen will lock once the device exceeds the configured distance.

## Architecture

The core logic is organized as a layered Rust Cargo workspace. Native interfaces for each platform live under `apps/`.
Dependencies flow from the interface → `runtime` → `platform` → `core`.

```
crates/
  awaylock-core/       Core: pure logic, no operating system API dependencies, with unit tests
    distance.rs          RSSI ↔ distance conversion
    device.rs            Detected devices and signal smoothing
    allowlist.rs         Device allowlist
    settings.rs          Settings (distance: 1–5 meters)
    monitor.rs           Lock decision state machine
    ports.rs             Ports: traits implemented by the platform layer
  awaylock-platform/   Platform: platform-specific implementations of core traits
    ble.rs               btleplug Bluetooth scanning (shared by macOS and Linux)
    locker.rs            Screen locking
    lockstate.rs         Querying whether the computer is locked
    autostart.rs         Launch at login
  awaylock-runtime/    Runtime: combines core and platform layers (background checks, settings storage, UI state views)
  awaylock-ffi/        C interface (dynamic library), passing state as JSON for Swift callers
  awaylock-app/        Alternative cross-platform interface (egui), reserved for Linux; unused on macOS
apps/
  macos/               Native macOS interface (SwiftUI / AppKit), calling Rust through the C interface
assets/AppIcon.icns    App icon (generated by scripts/make-icon.swift)
packaging/macos/       Info.plist for macOS packaging
scripts/               Packaging scripts, icon generation script, and early prototypes
```

| Capability | macOS | Linux (unverified) |
|---|---|---|
| Bluetooth scanning | btleplug (CoreBluetooth) | btleplug (BlueZ) |
| Screen locking | `pmset displaysleepnow` | `loginctl lock-session` |
| Lock state | `CGSessionCopyCurrentDictionary` | `loginctl show-session -p LockedHint` |
| Launch at login | `SMAppService` in Swift | XDG autostart |
| Interface | Native Swift (`apps/macos`) | egui (`crates/awaylock-app`) |

To support a new platform, add the corresponding implementations in `awaylock-platform`. The core decision logic does not need to change.

## Limitations
- **Apple device identifiers change periodically.** To prevent tracking, iPhone and Apple Watch BLE addresses rotate approximately every 15 minutes
  (the actual interval has not been measured). An allowlisted identifier will become invalid and must be added again; otherwise, the device will be considered missing and trigger a lock.
  Bluetooth accessories with fixed addresses (headphones, speakers, fitness bands, etc.) are unaffected. See [HANDOFF.md](HANDOFF.md).
- iPhone and Apple Watch cannot be paired with a Mac over Bluetooth, so signal readings from a paired connection are unavailable.
- The “phones and watches only” filter uses **device names** (containing iPhone, phone, or watch, while excluding names that resemble headphones, Macs, etc.).
  Unnamed devices (such as an iPhone after its identifier changes) are hidden because their advertisements do not distinguish a phone from an iPad or Mac.
  Regular users cannot disable this filter. In development builds, enabling Debug mode allows the Devices tab to show all devices.
- Distance is a rough estimate based on signal strength (approximately -59 dBm at 1 meter, with an environmental factor of 2.5). Walls, body obstruction, and carrying a phone in a pocket can all affect accuracy. Treat the distance as a reference only.
- The Linux version has not been built or tested. Its tray icon requires an additional extension on GNOME.

## License
Not yet specified.
