FROM rust:latest AS builder

# Create an empty crate with the same dependencies as the actual crate, then
# build the dependencies. The result will be cached by Docker, unless
# `Cargo.toml` or `Cargo.lock` change.
RUN cargo new model-api
WORKDIR model-api
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

# Now copy the actual source code and build it. Everything up until here won't
# have to be rebuilt if only the files in `src/` change, which is much more
# common than the dependencies changing.
COPY src ./src
RUN touch src/main.rs && cargo build --release


FROM archlinux:latest

RUN pacman -Sy && pacman -S openscad --noconfirm

COPY --from=builder /model-api/target/release/model-api model-api
COPY models ./models

ENV ROCKET_ADDRESS=::
ENV ROCKET_PORT=80
ENV ROCKET_LOG_LEVEL=normal

EXPOSE 80/tcp

CMD ["./model-api"]
