FROM simonkorl0228/aitrans_build:buster as build
WORKDIR /build
COPY ./dtp_client ./dtp_client
COPY ./dtp_server ./dtp_server
COPY ./dtp_utils ./dtp_utils
COPY ./deps ./deps
COPY ./Makefile ./Makefile
RUN echo "[source.crates-io]\n\
    replace-with = 'tuna'\n\n\
    [source.tuna]\n\
    registry = \"https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git\"" > $CARGO_HOME/config && \
    make all

FROM simonkorl0228/aitrans_image_base:buster
COPY --from=build \
    /build/dtp_server/target/release/dtp_server /home/aitrans-server/bin/server
COPY --from=build \
    /build/dtp_client/target/release/dtp_client /home/aitrans-server/client
