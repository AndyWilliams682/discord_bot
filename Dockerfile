FROM rust:1.80 as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

CMD ["discord_bot"]
