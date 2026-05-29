# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY migrations migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p afrasyab-app \
    && cp target/release/afrasyab /tmp/afrasyab

FROM debian:bookworm-slim AS runtime

# Pin: Deno 2.x (yt-dlp EJS); re-pin when upgrading base image.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        ffmpeg \
        python3 \
        unzip \
    && curl -fsSL https://deno.land/install.sh | DENO_INSTALL=/usr/local sh \
    && chmod +x /usr/local/bin/deno \
    && curl -fsSL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp \
    && printf '%s\n' '--remote-components' 'ejs:github' > /etc/yt-dlp.conf \
    && apt-get purge -y --auto-remove curl unzip \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/afrasyab /usr/local/bin/afrasyab

ENV RUST_LOG=info
ENV HOME=/tmp

USER nobody
CMD ["afrasyab"]
