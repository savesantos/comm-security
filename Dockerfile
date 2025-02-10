FROM rust:1.81

WORKDIR /workspace

RUN apt-get update && apt-get install -y less vim graphviz

RUN curl -L https://risczero.com/install | bash

RUN mkdir -p /root/.cargo/bin

RUN mkdir -p /projects

ENV PATH="$PATH:/root/.risc0/bin:/root/.cargo/bin:/root/go/bin"

RUN rustup component add rust-analyzer

RUN rzup install

WORKDIR /tmp

RUN wget https://go.dev/dl/go1.23.2.linux-amd64.tar.gz

RUN tar -C /root -xzf go1.23.2.linux-amd64.tar.gz

RUN rm go1.23.2.linux-amd64.tar.gz

RUN go install github.com/google/pprof@latest

WORKDIR /workspace
