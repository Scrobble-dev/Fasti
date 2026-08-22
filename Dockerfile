# syntax=docker/dockerfile:1

FROM rust:1.97-alpine AS rust-builder
RUN apk add --no-cache musl-dev
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY apps/fastid/ ./apps/fastid/
COPY xtask/ ./xtask/
COPY contracts/ ./contracts/

RUN cargo build --locked --release --bin fastid --bin fasti

FROM alpine:3.22 AS runtime
ARG FASTI_SOURCE_COMMIT=""
ARG FASTI_SOURCE_TREE=""
ARG FASTI_CONTRACT_REF=""
LABEL org.opencontainers.image.revision="${FASTI_SOURCE_COMMIT}" \
      dev.scrobble.fasti.source.tree="${FASTI_SOURCE_TREE}" \
      dev.scrobble.fasti.contracts="${FASTI_CONTRACT_REF}"
RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S fasti \
    && adduser -S fasti -G fasti

COPY --from=rust-builder /app/target/release/fastid /usr/local/bin/fastid
COPY --from=rust-builder /app/target/release/fasti /usr/local/bin/fasti

ENV FASTI_LISTEN=0.0.0.0:8420
EXPOSE 8420
USER fasti:fasti

HEALTHCHECK --interval=10s --timeout=3s --start-period=3s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8420/api/v1/health || exit 1

CMD ["/usr/local/bin/fastid"]
