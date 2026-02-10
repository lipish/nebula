FROM rust:1.76 as builder
WORKDIR /src
COPY . .
RUN cargo build -p nebula-scheduler --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /src/target/release/nebula-scheduler /app/nebula-scheduler
ENTRYPOINT ["/app/nebula-scheduler"]
