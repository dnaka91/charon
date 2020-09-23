# syntax = docker/dockerfile:experimental
FROM clux/muslrust:stable as builder

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/volume/target \
    cargo install --locked --path .

FROM alpine:3.12

WORKDIR /data

COPY --from=builder /root/.cargo/bin/charon /app/

EXPOSE 8080 8443
STOPSIGNAL SIGINT

ENTRYPOINT ["/app/charon"]
