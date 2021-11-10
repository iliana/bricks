FROM node:16-buster as css-builder
WORKDIR /usr/src/bricks
COPY . .
RUN npm ci
RUN npx postcss styles.css -o styles.min.css

FROM rust:1.56-buster as builder
WORKDIR /usr/src/bricks
COPY . .
COPY --from=css-builder /usr/src/bricks/styles.min.css styles.min.css
RUN COMPILED_CSS=styles.min.css cargo install --path .
RUN objcopy --compress-debug-sections /usr/local/cargo/bin/bricks

FROM debian:buster-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/bricks /usr/local/bin/bricks
CMD ["bricks"]
