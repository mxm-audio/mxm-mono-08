# mxm-mono-08

A patchable monophonic CLAP instrument with a complex oscillator, a modulation oscillator, two
low-pass gates, a five-step CV/trigger sequencer and a built-in mono spring.

The plugin supplies permanent parameters and state, note and performance handling, realtime
processing, telemetry, factory presets, controller data, and a resizable four-view editor. `Synth`
keeps the audio path visible, `Mod` exposes weighted CV and pulse routing without fake patch cables,
`Seq` keeps all five voltage/pulse stages, and `Parameters` exposes the complete host surface.

## Build

```bash
cargo xtask bundle mxm-mono-08 --release
```

The loadable bundle is written to `target/bundled/mxm-mono-08.clap` with `control-map.json` beside
it. It offers stereo and mono out, with no audio input.

## Licence

GPL-3.0-or-later. See the repository's [`LICENSE`](../../LICENSE).
