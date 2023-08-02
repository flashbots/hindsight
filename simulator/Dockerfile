FROM rust:slim-buster as builder
RUN apt-get update && apt-get install -y build-essential libssl-dev pkg-config
WORKDIR /app
COPY . .
RUN cargo build --release

FROM rust:slim-buster
WORKDIR /app
COPY --from=builder /app/target/release/hindsight .
ENTRYPOINT ["/app/hindsight"]
