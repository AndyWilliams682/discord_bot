FROM rust:1.66 as builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

CMD ["myapp"]
