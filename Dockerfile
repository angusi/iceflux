FROM debian:jessie

WORKDIR /opt/iceflux

COPY ./target/release/iceflux /opt/iceflux/iceflux

ENTRYPOINT ["/opt/iceflux/iceflux"]
