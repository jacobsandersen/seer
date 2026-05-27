FROM rust:1.95-trixie AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian13:nonroot AS final
COPY --from=builder /app/migrations /home/nonroot/migrations
COPY --from=builder /app/target/release/seer /home/nonroot/main
USER nonroot:nonroot
ENV TZ=UTC
ENTRYPOINT ["/home/nonroot/main"]