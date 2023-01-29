FROM mcr.microsoft.com/devcontainers/rust:0-1-bullseye
RUN apt-get update && apt-get install -y g++-arm-linux-gnueabihf
USER vscode
RUN rustup target add armv7-unknown-linux-gnueabihf
RUN cargo install cargo-deb cargo-watch