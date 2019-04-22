FROM debian:unstable

RUN apt-get update
RUN apt-get full-upgrade -y
RUN apt-get autoremove -y
RUN apt-get install git make lld musl-dev musl-tools curl -y

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf > rustup-init
RUN sh rustup-init -y
RUN rustup target add x86_64-unknown-linux-musl
ENV RUSTFLAGS="-Clinker=lld"

