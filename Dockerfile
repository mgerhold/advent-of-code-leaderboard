FROM rust:1.91-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app
COPY --from=builder /build/target/release/advent-of-code-leaderboard .
CMD ["./advent-of-code-leaderboard", "server", "config.toml"]
