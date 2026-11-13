# Production

## Build

```bash
moltn build
```

The result is a single file: `target/release/<name>` (`<name>.exe` on Windows). It contains the server, the pages, the assets (JavaScript, CSS, images built by Vite) and the server-side rendering engine.

## Run

```bash
PORT=8080 ./target/release/my-app
```

The server of a project created by `moltn new` listens on `127.0.0.1` and the port from the `PORT` variable (3000 by default). To accept outside connections (container, server), start it with `HOST=0.0.0.0` or put a reverse proxy in front.

The production machine needs **neither Node nor the `web/` folder**.

## Deploying with Docker (example)

```dockerfile
FROM rust:1.99 AS build
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && apt-get install -y nodejs \
 && npm install -g pnpm
WORKDIR /app
COPY . .
RUN cargo install --path /path/to/moltn/crates/moltn && moltn build

FROM debian:bookworm-slim
COPY --from=build /app/target/release/my-app /usr/local/bin/my-app
ENV HOST=0.0.0.0 PORT=8080
CMD ["my-app"]
```

Adapt as needed: until moltn is published, the moltn repository must be available in the build image.

## Check before shipping

```bash
moltn gen --check        # types up to date
pnpm --dir web typecheck # frontend consistent with the types
cargo test               # including the assert_valid pages test
```
