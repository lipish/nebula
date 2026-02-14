FROM rust:1.76 as builder
WORKDIR /src
COPY . .
RUN cargo build -p nebula-bff --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /src/target/release/nebula-bff /app/nebula-bff
EXPOSE 18090
ENTRYPOINT ["/app/nebula-bff"]
