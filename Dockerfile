FROM node:16-buster as js-builder
WORKDIR /usr/src/bricks
COPY . .
RUN npm ci
RUN npx postcss --env production styles.css -o styles.min.css

FROM rust:1.57-buster as builder
COPY --from=golang:1.17-buster /usr/local/go /usr/local/go
ENV PATH /usr/local/go/bin:$PATH
WORKDIR /usr/src/bricks
COPY . .
COPY --from=js-builder /usr/src/bricks/styles.min.css styles.min.css
COPY --from=js-builder /usr/src/bricks/node_modules/tablesort/dist node_modules/tablesort/dist
RUN COMPILED_CSS=styles.min.css cargo install --path .
RUN objcopy --compress-debug-sections /usr/local/cargo/bin/bricks

FROM debian:buster-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/bricks /usr/local/bin/bricks
CMD ["bricks"]
