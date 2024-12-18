FROM rust:1.83-bookworm as builder

RUN apt-get update \
    && apt-get install --no-install-recommends -y \
       build-essential \
       ca-certificates \
       curl \
       libfontconfig1-dev \
       libssl-dev \
       pkg-config \
       sudo \
    && apt -y autoremove \
    && apt clean \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-deb


# Build gossip
COPY ./ gossip/
WORKDIR gossip
RUN mkdir -p /gossip/artifacts
RUN cargo deb -p gossip
RUN ls /gossip/target/debian/

FROM scratch as custom-exporter
COPY --from=builder /gossip/target/debian/gossip*_amd64.deb .
