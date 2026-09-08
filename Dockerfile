# fast_code_search server image.
#
#   docker run --rm -p 8080:8080 -v "$PWD:/src:ro" ghcr.io/jburrow/fast_code_search
#
# Indexes /src (mount your tree there) and serves the web UI and REST API on
# :8080 and gRPC on :50051. Pass your own config with
# `-v ./config.toml:/etc/fast_code_search/config.toml:ro`; the index is
# persisted under /var/lib/fast_code_search (mount a volume to keep it).

FROM rust:1.98-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release --locked --bin fast_code_search_server --bin fcs

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /var/lib/fast_code_search --create-home fcs
COPY --from=build /build/target/release/fast_code_search_server /build/target/release/fcs /usr/local/bin/
COPY deploy/docker/config.toml /etc/fast_code_search/config.toml
USER fcs
VOLUME ["/var/lib/fast_code_search"]
EXPOSE 8080 50051
ENV FCS_SERVER=http://127.0.0.1:8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s CMD ["fcs", "status"]
ENTRYPOINT ["tini", "--", "fast_code_search_server", "--config", "/etc/fast_code_search/config.toml"]
