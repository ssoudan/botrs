FROM rust:bookworm AS chef
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG EXTRA_FEATURES=""

# Install dependencies
RUN apt-get update && apt-get install -y \
    libpython3-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY rust-toolchain.toml .
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json --features="$EXTRA_FEATURES"
# Build application
COPY . .
RUN cargo build --release --bin sapiens_cli --features="$EXTRA_FEATURES"
RUN cargo build --release --bin sapiens_bot --features="$EXTRA_FEATURES"

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS base-runtime
WORKDIR app

ARG USERNAME=not_me
ARG USER_UID=1000
ARG USER_GID=$USER_UID

# Create the user
RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME

# Install dependencies
RUN apt-get update && apt-get install -y \
    libpython3.11 \
    python3-sympy \
    python3-numpy \
    python3-requests \
    python3-urllib3 \
    python3-bs4 \
    python3-feedparser \
    python3-pip \
    python3-venv \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

USER $USERNAME
RUN python3 -m venv /home/$USERNAME/.venv
ENV PATH="/home/$USERNAME/.venv/bin:$PATH"
RUN pip3 install --no-cache-dir arxiv

FROM base-runtime AS sapiens_cli
ARG USER_UID=1000
ARG USER_GID=$USER_UID

COPY --from=builder /app/target/release/sapiens_cli /usr/local/bin

USER $USER_UID:$USER_GID

ENTRYPOINT ["/usr/local/bin/sapiens_cli"]

FROM base-runtime AS sapiens_bot
ARG USER_UID=1000
ARG USER_GID=$USER_UID

COPY --from=builder /app/target/release/sapiens_bot /usr/local/bin

USER $USER_UID:$USER_GID

ENTRYPOINT ["/usr/local/bin/sapiens_bot"]
