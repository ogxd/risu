FROM rust:1.77

WORKDIR /usr/src/risu
COPY . .

RUN RUSTFLAGS="-C target-feature=+aes" cargo install --path .

CMD ["risu"]