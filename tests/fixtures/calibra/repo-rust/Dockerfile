FROM rust:1.85-slim
WORKDIR /app
COPY . .
RUN cargo build --release
RUN useradd --create-home app
USER app
CMD ["cargo", "run", "--release"]
