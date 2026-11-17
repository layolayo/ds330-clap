# ds330-clap

A Linux **CLAP** plugin for controlling and automating the **Roland Dr. Synth DS-330** (General MIDI / Roland GS) via MIDI, NRPNs, and GS SysEx (designed for Bitwig Studio and other CLAP hosts).

## Features
* **16-Part Editor (`Part 1–16`):**
  * Bank Select (`CC 0`), Program Change (`1–128`) with GM tone names, and **GS Drum Kit** quick selector (`Standard`, `Room`, `Power`, `Electronic`, `TR-808`, `Jazz`, `Brush`, `Orchestra`, `SFX`)
  * Per-part **Mode** (`Normal`, `Drum1`, `Drum2`) via Roland GS SysEx
  * Live **TVF Filter** (`Cutoff`, `Resonance`), **TVA Envelope** (`Attack`, `Decay`, `Release`), and **Vibrato** (`Rate`, `Depth`, `Delay`, `Mod Wheel`) via Roland GS NRPNs (`CC 99=1`) with live visual graphs and per-section **Reset** buttons
  * Part **Mix & Sends** (`Volume`, `Pan`, `Reverb Send`, `Chorus Send`)
* **16-Part Mixer:** Side-by-side overview of all 16 MIDI channels (`Vol`, `Pan`, `Rev`, `Cho`, `Mute`)
* **Global FX & Master:** GS Master Volume, Master Pan, Key Shift, GS Reverb, and GS Chorus
* **SysEx & Backup (`.syx`):** Save/load `.syx` files in `~/DS330_Patches/`, **Push All 16 Parts**, and **SysEx Recorder** via ALSA Sequencer MIDI In
* **Direct ALSA Sequencer MIDI I/O:** Forwards Bitwig Notes, CCs, Pitch Bend, Program Changes, and Aftertouch (with optional `Route Notes -> Active Part`)

## Building on Linux

### Prerequisites
* Rust toolchain (`cargo`, `rustc`)
* ALSA development headers (`libasound2-dev` on Debian/Ubuntu) and `pkg-config`

### Build & Install
```bash
cargo xtask bundle ds330-clap --release
cp -r target/bundled/ds330-clap.clap ~/.clap/
```

## License
GPL-3.0-or-later
