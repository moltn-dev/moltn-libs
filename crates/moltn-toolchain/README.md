# moltn-toolchain

Detect, resolve and drive a project's JavaScript toolchain from Rust: Node, Bun or Deno; npm, pnpm, Yarn or bun.

- Detection from `package.json` (`packageManager`, `devEngines`, `engines`), lockfiles, `.nvmrc`, `.prototools`, `mise.toml`… up to the repository root.
- Resolution through proto, mise, Volta or the `PATH`, with actionable version errors.
- `install`, scripts, and supervised dev processes whose whole tree is killed on drop (process groups on Unix, Job Objects on Windows).

```rust
let tc = moltn_toolchain::JsProject::detect("web")?.resolve()?;
tc.run_script("build", &[]).await?;
```

## Part of moltn

Rust + Inertia v3 (Vue, React, Svelte) with server-side rendering inside the binary.
Guide (English and French): the `guide/` folder of the moltn repository.

## License

MIT OR Apache-2.0.
