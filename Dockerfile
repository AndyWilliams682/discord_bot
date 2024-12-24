FROM rust:1.71 as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

CMD ["myapp"]
