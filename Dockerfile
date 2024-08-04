FROM rustlang/rust:nightly

WORKDIR /tmp

# Install dependencies and Rust
RUN apt-get -qq update && \
    apt-get -y -qq install wget build-essential libssl-dev libuv1-dev && \
    # Download and install Cassandra C++ Driver
    wget -O cassandra-cpp-driver_2.17.1-1_amd64.deb https://datastax.jfrog.io/artifactory/cpp-php-drivers/cpp-driver/builds/2.17.1/e05897d/ubuntu/22.04/cassandra/v2.17.1/cassandra-cpp-driver_2.17.1-1_amd64.deb && \
    wget -O cassandra-cpp-driver-dev_2.17.1-1_amd64.deb https://datastax.jfrog.io/artifactory/cpp-php-drivers/cpp-driver/builds/2.17.1/e05897d/ubuntu/22.04/cassandra/v2.17.1/cassandra-cpp-driver-dev_2.17.1-1_amd64.deb && \
    dpkg -i cassandra-cpp-driver_2.17.1-1_amd64.deb cassandra-cpp-driver-dev_2.17.1-1_amd64.deb && \
    rm -f cassandra-cpp-driver_2.17.1-1_amd64.deb cassandra-cpp-driver-dev_2.17.1-1_amd64.deb && \
    apt-get clean

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY . .

RUN cargo build --release

ENV RUST_LOG=trace

EXPOSE 8080