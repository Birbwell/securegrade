FROM rust:alpine3.22

WORKDIR /app

COPY . /app

CMD ["cargo", "clean"]
CMD ["cargo", "run", "--release", "-q", "--offline"]