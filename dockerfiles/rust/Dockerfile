FROM rust:alpine3.22

WORKDIR /app

RUN <<EOF
cargo init --vcs=none 2&> /dev/null
rm src/main.rs
EOF

COPY ./submission /app/src

EXPOSE 80

CMD ["cargo", "run", "--release", "-q", "--offline"]