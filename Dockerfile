# syntax=docker/dockerfile:1

# Stage 1: Build Frontend
FROM node:20-alpine AS web-builder
WORKDIR /app
RUN corepack enable && corepack prepare pnpm@9 --activate
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml* ./
COPY packages/ ./packages/
COPY apps/web/ ./apps/web/
RUN pnpm install --frozen-lockfile || pnpm install
RUN pnpm --filter @fasti/web build || mkdir -p /app/apps/web/dist

# Stage 2: Build Rust Backend
FROM rust:1.80-alpine AS rust-builder
RUN apk add --no-cache musl-dev sqlite-dev
WORKDIR /app
COPY Cargo.toml ./
COPY crates/ ./crates/
COPY apps/fastid/ ./apps/fastid/
RUN cargo build --release --bin fastid

# Stage 3: Runtime
FROM alpine:3.20 AS runtime
RUN apk add --no-cache ca-certificates tzdata sqlite-libs
WORKDIR /app

# Create non-root user
RUN addgroup -S fasti && adduser -S fasti -G fasti
USER fasti:fasti

COPY --from=rust-builder /app/target/release/fastid /usr/local/bin/fastid
COPY --from=web-builder /app/apps/web/dist /app/static

ENV FASTI_PORT=8420
ENV FASTI_STATIC_DIR=/app/static
EXPOSE 8420

ENTRYPOINT ["fastid"]
