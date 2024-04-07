FROM rust:1.77

WORKDIR /usr/src/risu
COPY . .

RUN cargo install --path .

CMD ["risu"]