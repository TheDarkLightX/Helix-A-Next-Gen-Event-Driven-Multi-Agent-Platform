FROM node:22-bookworm-slim AS ui-builder
WORKDIR /workspace/ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui ./
RUN npm run build

FROM rust:1-bookworm AS api-builder
WORKDIR /workspace
COPY Cargo.toml ./
COPY crates ./crates
RUN cargo build --release -p helix-api

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/helix
COPY --from=api-builder /workspace/target/release/helix-api /usr/local/bin/helix-api
COPY --from=ui-builder /workspace/ui/dist /opt/helix/ui/dist

ENV HELIX_API_ADDR=0.0.0.0:3000
ENV HELIX_UI_DIST=/opt/helix/ui/dist
EXPOSE 3000

CMD ["helix-api"]
