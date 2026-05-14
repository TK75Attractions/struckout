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