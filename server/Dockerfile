FROM registry.access.redhat.com/ubi10/ubi-minimal:10.2

ARG SHRIMPLY_SERVER_GIT_HASH=""
ENV SHRIMPLY_SERVER_GIT_HASH=${SHRIMPLY_SERVER_GIT_HASH}

COPY --from=ghcr.io/astral-sh/uv:0.12.6 /uv /usr/local/bin/uv
# The PyCOLMAP CUDA wheel links directly to these X11 runtime libraries.
# Git and GCC are required to build commit-pinned Python dependencies.
RUN microdnf install -y --setopt=install_weak_deps=0 gcc-c++ git-core libSM libX11 libXext \
    && microdnf clean all
WORKDIR /app
COPY . .

CMD ["uv", "run", "--locked", "--no-dev", "--exact", "src/main.py"]
