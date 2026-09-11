FROM public.ecr.aws/d3j8x8q7/olympus-base:latest

WORKDIR /app

COPY . .

RUN cargo fetch --locked \
    && cargo build --workspace --locked \
    && cargo install cargo-nextest --version 0.9.100 --locked

CMD ["/bin/bash"]
