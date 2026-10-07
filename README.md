# project butterfly

playing synths with a drawing tablet instead of a keyboard.

tablets already have solid continuous tracking and pressure sensitivity, so this turns stylus input into sound in real time:
- stylus position controls pitch across scales
- pen pressure shapes dynamics, timbre, and modulation
- audio runs through jack and lv2 plugins, separate from the egui canvas


[demo](https://github.com/user-attachments/assets/cb14a0a1-f377-41ee-8b24-35cbdcb11a1a
)

[youtube version](https://www.youtube.com/watch?v=ZB5CRUAWFpI)





## running
Selection deleted

requires a running jack / pipewire-jack setup and a connected drawing tablet:

```bash
cargo run --release
```

## notes

originally hacked together for cseed's buildspace. the winit/wgpu/egui setup was based on [kaphula's template](https://github.com/kaphula/winit-egui-wgpu-template).
