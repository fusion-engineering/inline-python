ARG VARIANT="buster"
FROM mcr.microsoft.com/vscode/devcontainers/rust:0-${VARIANT}

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends libpython3.9-dev

RUN rustup toolchain install nightly
RUN rustup default nightly
RUN rustup component add rust-src rustfmt clippy
