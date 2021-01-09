FROM rust:1.49 AS builder
WORKDIR /workdir                       
ENV CARGO_HOME=/workdir/.cargo                       
COPY ./Cargo.toml ./Cargo.lock ./                       
COPY ./src ./src
RUN cargo build --release

FROM ubuntu:20.04
COPY --from=0 /workdir/target/release/imagepullsecret-sync /usr/local/bin
ENTRYPOINT ["imagepullsecret-sync"]