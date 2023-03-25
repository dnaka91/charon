FROM rust:1.68 as builder

WORKDIR /volume

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools && \
    rustup target add x86_64-unknown-linux-musl && \
    cargo init --bin

COPY Cargo.lock Cargo.toml ./

RUN cargo build --release --target x86_64-unknown-linux-musl

COPY src/ src/

RUN touch src/main.rs && cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3 as newuser

RUN echo "charon:x:1000:" > /tmp/group && \
    echo "charon:x:1000:1000::/dev/null:/sbin/nologin" > /tmp/passwd

FROM scratch

WORKDIR /data

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/charon /bin/
COPY --from=newuser /tmp/group /tmp/passwd /etc/

EXPOSE 8080 8443
STOPSIGNAL SIGINT
USER charon

ENTRYPOINT ["/bin/charon"]
