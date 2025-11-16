# --- STAGE 1: BUILD ---
FROM rust:1.83 as builder

WORKDIR /usr/src/app
ENV CARGO_TARGET_DIR=/usr/src/app/target

# 1. Dependency Caching: Copy manifest and setup dummy structure
COPY Cargo.toml ./

# 🛑 FIX: Ensure the source directory is created BEFORE creating the dummy file
RUN mkdir -p src/
RUN echo "fn main() {}" > src/main.rs

# Now run the build to cache dependencies
RUN cargo build --release

# 2. Source Code Copy: Overwrite the dummy file with the actual source
COPY . .

# 3. Final Build: Compile the actual binary
# You no longer need `rm src/main.rs` because the `COPY . .` step overwrote the dummy file.
RUN cargo build --release --bin discord_bot

# --- STAGE 2: FINAL IMAGE (Small) ---
FROM debian:stable-slim

WORKDIR /usr/local/bin

COPY config.json /usr/local/bin/

# Copy the optimized binary from the builder stage
COPY --from=builder /usr/src/app/target/release/discord_bot /usr/local/bin/discord_bot

CMD ["discord_bot"]