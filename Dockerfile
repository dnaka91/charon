FROM rust:1.56-alpine as builder

WORKDIR /volume

RUN apk add --no-cache build-base=~0.5 musl-dev=~1.2 openssl-dev=~1.1

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN cargo build --release && \
    strip --strip-all target/release/charon

FROM alpine:3.14 as newuser

RUN echo "charon:x:1000:" > /tmp/group && \
    echo "charon:x:1000:1000::/dev/null:/sbin/nologin" > /tmp/passwd

FROM scratch

WORKDIR /data

COPY --from=builder /volume/target/release/charon /bin/
COPY --from=newuser /tmp/group /tmp/passwd /etc/

EXPOSE 8080 8443
STOPSIGNAL SIGINT
USER charon

ENTRYPOINT ["/bin/charon"]
