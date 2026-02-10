FROM rust:1.76 as builder
WORKDIR /src
COPY . .
RUN cargo build -p nebula-router --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /src/target/release/nebula-router /app/nebula-router
EXPOSE 18081
ENTRYPOINT ["/app/nebula-router"]
