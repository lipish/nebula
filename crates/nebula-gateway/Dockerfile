FROM rust:1.76 as builder
WORKDIR /src
COPY . .
RUN cargo build -p nebula-gateway --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /src/target/release/nebula-gateway /app/nebula-gateway
EXPOSE 8081
ENTRYPOINT ["/app/nebula-gateway"]
