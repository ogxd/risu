FROM rust:1.77-slim-buster as builder

WORKDIR /usr/src/risu
COPY . .

RUN RUSTFLAGS="-C target-feature=+aes" cargo install --path .

FROM debian:buster-slim

COPY --from=builder /usr/local/cargo/bin/risu /usr/local/bin/risu

CMD ["risu"]