FROM rust:1.69.0

WORKDIR /app

RUN apt update && apt install lld clang -y

COPY . .

# to update sqlx-data.json run `cargo sqlx prepare -- --lib`
ENV SQLX_OFFLINE true
ENV APP_ENVIRONMENT production

RUN cargo build --release

ENTRYPOINT ["./target/release/zero2prod"]