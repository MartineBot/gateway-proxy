# I will only support x86_64 hosts because this allows for best hardware optimizations.
# Compiling a Dockerfile for aarch64 should work but I won't support it myself.
ARG TARGET_CPU="haswell"

FROM docker.io/library/alpine:edge AS builder
ARG TARGET_CPU
ENV RUST_TARGET "x86_64-unknown-linux-musl"
ENV RUSTFLAGS "-Lnative=/usr/lib -C target-cpu=${TARGET_CPU}"

RUN apk upgrade && \
    apk add curl build-base g++ cmake make && \
    curl -sSf https://sh.rustup.rs | sh -s -- --profile minimal --component rust-src --default-toolchain nightly -y

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo/

RUN mkdir src/
RUN echo 'fn main() {}' > ./src/main.rs
RUN source $HOME/.cargo/env && \
    if [ "$TARGET_CPU" == 'x86-64' ]; then \
        cargo build --release --target="$RUST_TARGET" --no-default-features --features no-simd; \
    else \
        cargo build --release --target="$RUST_TARGET"; \
    fi

RUN rm -f target/$RUST_TARGET/release/deps/gateway_proxy*
COPY ./src ./src

RUN source $HOME/.cargo/env && \
    if [ "$TARGET_CPU" == 'x86-64' ]; then \
        cargo build --release --target="$RUST_TARGET" --no-default-features --features no-simd; \
    else \
        cargo build --release --target="$RUST_TARGET"; \
    fi && \
    cp target/$RUST_TARGET/release/gateway-proxy /gateway-proxy && \
    strip /gateway-proxy

FROM docker.io/library/alpine:edge

COPY --from=builder /gateway-proxy /gateway-proxy

RUN apk upgrade && apk add curl

HEALTHCHECK --interval=5s CMD curl --fail http://0.0.0.0:5421/health || exit 1

CMD ["./gateway-proxy"]
