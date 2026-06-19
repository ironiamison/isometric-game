# Solstead — single Railway service (nginx + static site + game server)
#
# Build args (set in Railway → Service → Variables → build-time):
#   PUBLIC_URL=https://solstead.xyz
#   AEVEN_WS_URL=wss://solstead.xyz

ARG PUBLIC_URL=https://solstead.xyz
ARG AEVEN_WS_URL=wss://solstead.xyz
ARG SOLSTEAD_MINT_ADDRESS=Ez1JTZYnPJicwV4rhtfuhUPnyQHMaBQAXNksPbZ9pump

# ── WASM client ──────────────────────────────────────────────────────────────
FROM rust:1.92-bookworm AS wasm-build
ARG PUBLIC_URL
ARG AEVEN_WS_URL
WORKDIR /app
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY client ./client
COPY launcher ./launcher
COPY rust-server ./rust-server
RUN rustup target add wasm32-unknown-unknown
ENV AEVEN_SERVER_URL=${PUBLIC_URL} \
    AEVEN_WS_URL=${AEVEN_WS_URL} \
    AEVEN_ALLOW_INSECURE_ENDPOINTS=0
RUN cargo build --locked --target wasm32-unknown-unknown --profile release-wasm -p new-aeven-client
RUN mkdir -p /play && \
    WASM_DIR=/app/target/wasm32-unknown-unknown/release-wasm && \
    if [ -f "$WASM_DIR/isometric_client.wasm" ]; then \
      cp "$WASM_DIR/isometric_client.wasm" /play/; \
    else \
      cp "$WASM_DIR/libisometric_client.wasm" /play/isometric_client.wasm; \
    fi && \
    cp client/web/*.js client/web/index.html /play/ && \
    cp client/web/*.css /play/ 2>/dev/null || true && \
    cp -R client/assets/. /play/assets/ && \
    mkdir -p /play/assets/title && \
    cp -R client/assets/title/. /play/assets/title/

# ── SvelteKit static site ────────────────────────────────────────────────────
FROM node:22-bookworm AS site-build
ARG PUBLIC_URL
ARG SOLSTEAD_MINT_ADDRESS=Ez1JTZYnPJicwV4rhtfuhUPnyQHMaBQAXNksPbZ9pump
WORKDIR /app/site
COPY site/package.json site/package-lock.json ./
RUN npm ci
COPY site/ ./
COPY rust-server/data ../rust-server/data
COPY rust-server/maps ../rust-server/maps
COPY --from=wasm-build /play ./static/play
ENV VITE_SITE_URL=${PUBLIC_URL}
ENV VITE_SOLSTEAD_MINT_ADDRESS=${SOLSTEAD_MINT_ADDRESS}
RUN npm run build

# ── Rust game server ─────────────────────────────────────────────────────────
FROM rust:1.92-bookworm AS server-build
WORKDIR /app
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY client ./client
COPY launcher ./launcher
COPY rust-server ./rust-server
RUN cargo build --locked --release -p isometric-server

# ── Runtime ──────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    nginx gettext-base ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/rust-server
COPY --from=server-build /app/target/release/isometric-server ./isometric-server
COPY rust-server/data ./data
COPY rust-server/maps ./maps
COPY --from=site-build /app/site/build /var/www/solstead

COPY deploy/nginx-railway.conf.template /etc/nginx/templates/default.conf.template
COPY deploy/start-railway.sh /app/start-railway.sh
RUN chmod +x /app/start-railway.sh && mkdir -p /data /var/log/nginx

ENV AEVEN_BIND_ADDR=127.0.0.1:2567 \
    AEVEN_DATABASE_URL=sqlite:/data/game.db?mode=rwc \
    AEVEN_ENV=production \
    PORT=8080

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=10s --start-period=300s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PORT}/health" || exit 1

CMD ["/app/start-railway.sh"]
