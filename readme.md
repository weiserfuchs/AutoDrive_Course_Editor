# AutoDrive Course Editor

An editor for the route networks (`AutoDrive_config.xml`) of the AutoDrive mod for Farming Simulator. This is a Rust rewrite of the former Java editor; it handles large networks smoothly.

## Download

Download `AutoDriveCourseEditor.exe` from the [releases page](https://github.com/weiserfuchs/AutoDrive_Course_Editor/releases) and start it. There is nothing to install and no Java is needed (Windows x64).

At start the editor looks for a newer release and tells you once per version. You can turn this off under Edit ▸ Settings… ▸ Editor.

## Compatibility

The editor works with configs written by the [FS19](https://github.com/Stephan-S/FS19_AutoDrive), [FS22](https://github.com/Stephan-S/FS22_AutoDrive) and [FS25](https://github.com/Stephan-S/FS25_AutoDrive) versions of AutoDrive. Configs older than AutoDrive 1.1.0.6 open read-only. It also opens and saves the route files of the AutoDrive route manager (File ▸ Routes…).

When you save next to a savegame's `vehicles.xml`, the editor keeps the vehicles' park destinations in step with the markers.

## Settings and logs

- Settings, recent files and key bindings: `%APPDATA%\AutoDrive Course Editor\config\settings.toml`. Edit them in Edit ▸ Settings….
- Logs: `%APPDATA%\AutoDrive Course Editor\data\logs\`; Help ▸ Open log folder opens it.
- Autosave writes rotating `<config>_autosave_<n>.xml` files next to the open config. It never changes the file itself.

## Map images

The editor downloads map images from the [AutoDrive_MapImages](https://github.com/KillBait/AutoDrive_MapImages) repository when needed (Edit ▸ Settings… ▸ Editor). You can also import one from a map mod zip or a DDS file (Import menu). There is a [guide to creating map images](https://github.com/KillBait/AutoDrive_MapImages/discussions/20).

If a map is missing from that repository, consider a pull request there, or ask in its [discussions](https://github.com/KillBait/AutoDrive_MapImages/discussions).

## Bug reports

Please use the [issues](https://github.com/weiserfuchs/AutoDrive_Course_Editor/issues) of this repository. Attach the log file if the editor misbehaved.

## Building from source

Install [Rust](https://rustup.rs/); `rust-toolchain.toml` picks the version. Then:

```
cargo run -p ad-app --release
```

The checks are `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.

## License

The editor does not currently have a license attached to it.

## Credits

- KillBait, author of the Java editor this one is ported from.
- Stephan, creator of the AutoDrive mod and the original editor.
- rhaetional and whitevamp, for code in the Java editor.
- Everybody who sent suggestions and bug reports.
