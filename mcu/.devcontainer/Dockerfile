FROM rust:latest

ARG USER_NAME=vscode
ARG USER_UID=1000
ARG USER_GID=1000

RUN apt update -y\
    && apt install -y sudo
RUN groupadd -g ${USER_GID} ${USER_NAME} \
    && useradd -m -u ${USER_UID} -g ${USER_GID} -s /bin/bash -G sudo,plugdev ${USER_NAME} \
    && echo '%sudo ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

USER ${USER_NAME}

RUN curl -L --proto '=https' --tlsv1.2 -sSf\
    https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

RUN cargo binstall -y espup \
    && espup install -t esp32,esp32s3

RUN cargo binstall -y probe-rs-tools --bin probe-rs \
    && cargo binstall espflash

USER root

RUN cp /home/${USER_NAME}/export-esp.sh /etc/profile.d/
RUN usermod -a -G plugdev ${USER_NAME}
    
