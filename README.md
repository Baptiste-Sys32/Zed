# Zed (Custom Build)

This is a custom branch of [Zed](https://github.com/zed-industries/zed) maintained by [Baptiste-Sys32](https://github.com/Baptiste-Sys32).

It integrates custom user interface components and extensions tailored for experimental features:
- **`brave_ui`**: Integration for specialized Brave UI views and runtime.
- **`codex_ui`**: Custom workspace panels, profiles, and app server configurations.

---

### Key Additions
- Custom Linux, macOS, and Windows keymaps under `assets/keymaps/`
- Custom icon set updates (including `assets/icons/brave.svg`)
- Initialized `brave_ui` and `codex_ui` crates in the gpui application lifecycle (`crates/zed/src/main.rs`)

### Development & Building

To compile and run this custom build locally:

```bash
cargo run --release
```

For platform-specific build instructions, refer to:
- [Building on Linux](./docs/src/development/linux.md)
- [Building on macOS](./docs/src/development/macos.md)
- [Building on Windows](./docs/src/development/windows.md)

### Licensing

Inherited from the upstream repository, the source code is licensed primarily under the **GPL-3.0-or-later**, with select components under **Apache-2.0**.
