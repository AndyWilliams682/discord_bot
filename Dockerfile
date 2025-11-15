FROM rust:1.83 as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

CMD ["discord_bot"]
