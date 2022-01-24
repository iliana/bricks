# syntax = docker/dockerfile:1.3

FROM node:16-buster as js-builder
WORKDIR /usr/src/bricks
COPY . .
RUN --mount=type=cache,target=/root/.npm \
  npm ci && npx postcss --env production styles.css -o styles.min.css

FROM rust:1.58-buster as twemoji
RUN svn export https://github.com/twitter/twemoji/tags/v13.1.0/assets/svg twemoji

FROM rust:1.58-buster as builder
COPY --from=golang:1.17-buster /usr/local/go /usr/local/go
ENV PATH /usr/local/go/bin:$PATH
WORKDIR /usr/src/bricks
COPY . .
COPY --from=js-builder /usr/src/bricks/styles.min.css styles.min.css
COPY --from=js-builder /usr/src/bricks/node_modules/tablesort/dist node_modules/tablesort/dist
ARG GITHUB_SHA
RUN --mount=type=cache,target=/usr/src/bricks/target \
  --mount=type=cache,target=/usr/local/cargo/registry \
  COMPILED_CSS=styles.min.css cargo install --path .
RUN objcopy --compress-debug-sections /usr/local/cargo/bin/bricks

FROM debian:buster-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/bricks /bricks
COPY --from=twemoji /twemoji /twemoji
ENV ROCKET_ADDRESS=0.0.0.0 \
    TWEMOJI_SVG=/twemoji
CMD ["/bricks"]
