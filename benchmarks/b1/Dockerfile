# Multi-architecture Docker Official Image index digests resolved on 2026-08-22.
# The tags remain human-readable; the digests make the build inputs immutable.
FROM rust:1.97-alpine@sha256:3c38f3f82c2f3d73da3b38e18d279393a04cb43ddded0e35088a8c3324d40900 AS rust-builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY apps/fastid/ ./apps/fastid/
COPY xtask/ ./xtask/
COPY contracts/ ./contracts/

RUN cargo build --locked --release --bin fastid --bin fasti

FROM alpine:3.22@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce AS runtime
ARG FASTI_SOURCE_COMMIT
ARG FASTI_SOURCE_TREE
ARG FASTI_CONTRACT_REF
ARG FASTI_BUILD_RECIPE_SHA256
ARG FASTI_BUILD_CONTEXT_ARCHIVE_SHA256
RUN test -n "${FASTI_SOURCE_COMMIT}" \
    && test -n "${FASTI_SOURCE_TREE}" \
    && test -n "${FASTI_CONTRACT_REF}" \
    && test -n "${FASTI_BUILD_RECIPE_SHA256}" \
    && test -n "${FASTI_BUILD_CONTEXT_ARCHIVE_SHA256}"
LABEL org.opencontainers.image.revision="${FASTI_SOURCE_COMMIT}" \
      dev.scrobble.fasti.source.tree="${FASTI_SOURCE_TREE}" \
      dev.scrobble.fasti.contracts="${FASTI_CONTRACT_REF}" \
      dev.scrobble.fasti.build.recipe.sha256="${FASTI_BUILD_RECIPE_SHA256}" \
      dev.scrobble.fasti.build.context.archive.sha256="${FASTI_BUILD_CONTEXT_ARCHIVE_SHA256}"

COPY --from=rust-builder /app/target/release/fastid /usr/local/bin/fastid
COPY --from=rust-builder /app/target/release/fasti /usr/local/bin/fasti

ENV FASTI_LISTEN=0.0.0.0:8420
EXPOSE 8420
USER 65532:65532

CMD ["/usr/local/bin/fastid"]
