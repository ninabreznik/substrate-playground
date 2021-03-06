# A multi-stage docker image (https://docs.docker.com/develop/develop-images/multistage-build/)
# Based on https://github.com/bjornmolin/rust-minimal-docker

##########################
#         Frontend       #
##########################

FROM node:alpine AS builder-frontend

WORKDIR /opt

COPY frontend .

ARG ENVIRONMENT
ENV PARCEL_WORKERS=1, NODE_ENV=$ENVIRONMENT

RUN apk add --no-cache --virtual .gyp \
        python \
        make \
        g++ \
    && yarn global add parcel-bundler && yarn clean && yarn install && yarn build \
    && apk del .gyp

LABEL stage=builder

##########################
#         Backend        #
##########################

FROM clux/muslrust:nightly AS builder-backend

WORKDIR /opt

ENV BINARY_NAME=playground

# Build the project with target x86_64-unknown-linux-musl

# Build dummy main with the project's Cargo lock and toml
# This is a docker trick in order to avoid downloading and building 
# dependencies when lock and toml not is modified.

COPY backend/Cargo.* ./

RUN mkdir src \
    && echo "fn main() {print!(\"Dummy main\");} // dummy file" > src/main.rs \
    && set -x && cargo build --target x86_64-unknown-linux-musl --release \
    && set -x && rm target/x86_64-unknown-linux-musl/release/deps/$BINARY_NAME*

# Now add the rest of the project and build the real main

COPY backend/src src
COPY backend/conf /opt/conf

RUN set -x && cargo build --frozen --release --out-dir=/opt/bin -Z unstable-options --target x86_64-unknown-linux-musl

LABEL stage=builder

##########################
#         Runtime        #
##########################

FROM scratch

ENV RUST_BACKTRACE=full\
    RUST_LOG="error,$BINARY_NAME=info"

COPY --from=builder-backend /opt/bin/$BINARY_NAME /
COPY --from=builder-backend /opt/conf /conf
COPY --from=builder-frontend /opt/dist/ /static

CMD ["/playground"]