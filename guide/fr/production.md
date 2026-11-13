# Production

## Construire

```bash
moltn build
```

Le résultat est un seul fichier : `target/release/<nom>` (`<nom>.exe` sous Windows). Il contient le serveur, les pages, les assets (JavaScript, CSS, images construits par Vite) et le moteur de rendu serveur.

## Lancer

```bash
PORT=8080 ./target/release/mon-app
```

Le serveur d'un projet créé par `moltn new` écoute sur `127.0.0.1` et le port de la variable `PORT` (3000 par défaut). Pour accepter les connexions extérieures (conteneur, serveur), lancez-le avec `HOST=0.0.0.0` ou placez un proxy inverse devant.

La machine de production n'a besoin **ni de Node, ni du dossier `web/`**.

## Déployer avec Docker (exemple)

```dockerfile
FROM rust:1.99 AS build
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && apt-get install -y nodejs \
 && npm install -g pnpm
WORKDIR /app
COPY . .
RUN cargo install --path /chemin/vers/moltn/crates/moltn-cli && moltn build

FROM debian:bookworm-slim
COPY --from=build /app/target/release/mon-app /usr/local/bin/mon-app
ENV HOST=0.0.0.0 PORT=8080
CMD ["mon-app"]
```

À adapter : tant que moltn n'est pas publié, le dépôt moltn doit être disponible dans l'image de construction.

## Vérifier avant de livrer

```bash
moltn gen --check        # types à jour
pnpm --dir web typecheck # front cohérent avec les types
cargo test               # dont le test assert_valid des pages
```
