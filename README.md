# firefly-hal

Hardware Abstraction Layer for Firefly Zero device and emulators.

This package is used by firefly-runtime. It provides a `Device` trait and 3 separate implementations for it. The correct implementation is picked automatically based on the build target.

## Installation

Install from git. We use dev version of embedded-sdmmc so the project cannot yet be published to crates.io.

```toml
firefly-hal = { git = "https://github.com/firefly-zero/firefly-hal" }
```

## Development

Run all code formatters, linters, and tests for all environments:

1. [Install task](https://taskfile.dev/)
1. `task all`

Since there are 3 implementations that cannot coexist in the same environment, your IDE can only analyze one of them.

### Embedded

Install ESP32-compatible Rust fork. Follow instructions here: [RISC-V and Xtensa Targets](https://docs.esp-rs.org/book/installation/riscv-and-xtensa.html).

### Hosted

```bash
rm rust-toolchain.toml
rm -r .cargo
rm -r .vscode
```

When contributing, make sure to not commit removed files.

### Web

Follow the instructions for hosted and then in `lib.rs` replace `path = "hosted.rs"` with `path = "web.rs"`.

Keep in mind that web environment is currently not complete and cannot yet be compiled.

## License

Unlike SDKs, CLI, and other gamedev tools we provide (which are all under MIT License), the HAL, runtime, and emulators are licensed under GNU GPL. You can make your own Firefly Zero but it must be truly FOSS with all proper attributions.
