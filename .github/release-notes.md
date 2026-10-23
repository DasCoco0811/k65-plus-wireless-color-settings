Custom reactive per-key lighting for the **Corsair K65 PLUS WIRELESS** on macOS and Windows.

## Download

| System | File |
|---|---|
| Windows 10/11 (installer) | `K65-Plus-Wireless-Color-Settings-…-windows-x64-setup.exe` |
| Windows 10/11 (single file, no install) | `K65-Plus-Wireless-Color-Settings-…-windows-x64-portable.exe` |
| macOS 11+ (Apple Silicon and Intel) | `K65-Plus-Wireless-Color-Settings-…-macos-universal.dmg` |

`SHA256SUMS.txt` lists the checksums of all files.

## Before you start

- **iCUE must be installed and running.** Allow SDK / third-party control in the iCUE settings.
- The app starts at login automatically from its first start (switch off with *Start at login*). On macOS, start it from the Applications folder, not from the disk image.
- On first start, the app downloads Corsair's iCUE SDK once from [Corsair's official GitHub release](https://github.com/CorsairOfficial/cue-sdk/releases/tag/v4.0.84) and verifies its SHA-256 checksum. Corsair's license does not allow bundling it.
- The app is **not signed** with a paid certificate:
  - **macOS:** Move the app to Applications. Open it with right-click → Open, or go to *System Settings → Privacy & Security → Open Anyway*. Allow *Input Monitoring* when asked.
  - **Windows:** If SmartScreen appears, choose *More info → Run anyway*.
