# moltn-cli

The `moltn` command: create, develop, build and check a Rust + Inertia application built with the moltn libraries. The crate is named `moltn-cli`; the installed command is `moltn`.

```bash
cargo install moltn-cli
moltn new my-app --front vue   # or react, svelte
cd my-app
moltn dev                      # Vite + the Rust app, hot reload, restart on Rust changes
moltn build                    # one release binary, no Node needed to run it
```

Also: `moltn gen [--check]` (TypeScript types from Rust) and `moltn doctor` (toolchain diagnosis).

## Part of moltn

Rust + Inertia v3 (Vue, React, Svelte) with server-side rendering inside the binary.
Guide (English and French): <https://docs.moltn.eu>. Source: <https://github.com/moltn-dev/moltn-libs>.

## License

MIT OR Apache-2.0.
