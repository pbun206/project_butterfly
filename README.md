# project butterfly

playing synths with a drawing tablet instead of a keyboard.

tablets already have solid continuous tracking and pressure sensitivity, so this turns stylus input into sound in real time:
- stylus position controls pitch across scales
- pen pressure shapes dynamics, timbre, and modulation
- audio runs through jack and lv2 plugins, separate from the egui canvas


demo:
<video src="assets/demo.mp4" controls="controls" width="100%"></video>


## running

requires a running jack / pipewire-jack setup and a connected drawing tablet:

```bash
cargo run --release
```

## notes

originally hacked together for cseed's buildspace. the winit/wgpu/egui setup was based on [kaphula's template](https://github.com/kaphula/winit-egui-wgpu-template).
