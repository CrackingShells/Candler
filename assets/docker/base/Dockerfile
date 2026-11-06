# The agent base image: Python 3.12 on Debian bookworm, uv, and the Claude Code binary.
# Every input is pinned by digest or checked by sha256; nothing here is resolved at build time.
# Debian (glibc) rather than Alpine: coreutils `timeout` exits 124 on expiry, and python ships
# inside the digest-pinned image.
FROM python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df
ARG TARGETARCH
# The version and its checksums go together: change both or neither.
ARG CC_VERSION=2.1.311
RUN useradd -m -u 10001 -d /home/agent agent
COPY --from=ghcr.io/astral-sh/uv:0.9@sha256:538e0b39736e7feae937a65983e49d2ab75e1559d35041f9878b7b7e51de91e4 /uv /uvx /usr/local/bin/
# One layer holds the binary: no chmod or mv after this line, either would copy 290 MB again.
ADD --chmod=755 https://downloads.claude.ai/claude-code-releases/${CC_VERSION}/linux-${TARGETARCH/amd64/x64}/claude /usr/local/bin/claude
RUN case "${TARGETARCH}" in \
      arm64) sum=2e2496c9a8065df93acb0fca2d341f59a6b8777649b2e025ab8c9bcd5eef61f3 ;; \
      amd64) sum=86dc1fcc69498159eaf15f446613e847010f9036bfbbdffd4c49350be8489ada ;; \
      *) echo "no Claude Code checksum for ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
 && echo "${sum}  /usr/local/bin/claude" | sha256sum -c -
LABEL candler.cc_version=${CC_VERSION}
USER agent
ENV HOME=/home/agent
WORKDIR /home/agent
