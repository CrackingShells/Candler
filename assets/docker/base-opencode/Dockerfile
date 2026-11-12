# The OpenCode agent base image: the same Debian base, uv and agent user as assets/docker/base,
# with the OpenCode V2 binary in place of Claude Code.
# Every input is pinned by digest or checked by sha256; nothing here is resolved at build time.
# Debian (glibc) rather than Alpine: coreutils `timeout` exits 124 on expiry, and python ships
# inside the digest-pinned image.

# The release archive is fetched and checked in its own stage, so the image holds only the binary.
FROM python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df AS fetch
ARG TARGETARCH
# The version and its checksums go together: change both or neither. The checksums are those of
# the release archives (`opencode-linux-<arch>.tar.gz`); the x64 one is the digest the release
# publishes for its asset.
ARG OC_VERSION=2.2.7
ADD https://github.com/anomalyco/opencode/releases/download/v${OC_VERSION}/opencode-linux-${TARGETARCH/amd64/x64}.tar.gz /tmp/opencode.tar.gz
RUN case "${TARGETARCH}" in \
      arm64) sum=c1682057f0815cb2abdd6e00305f1146ebaf1c167f0335cdb74ddc80008120e7 ;; \
      amd64) sum=89a6cee227b54bc011179c528a5e63176032462b1a5e21723b616801cddc2ca7 ;; \
      *) echo "no OpenCode checksum for ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
 && echo "${sum}  /tmp/opencode.tar.gz" | sha256sum -c - \
 && mkdir /out \
 && tar -xzf /tmp/opencode.tar.gz -C /out opencode

FROM python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df
ARG OC_VERSION=2.2.7
RUN useradd -m -u 10001 -d /home/agent agent
COPY --from=ghcr.io/astral-sh/uv:0.9@sha256:538e0b39736e7feae937a65983e49d2ab75e1559d35041f9878b7b7e51de91e4 /uv /uvx /usr/local/bin/
# One layer holds the binary: no chmod or mv after this line, either would copy 250 MB again.
COPY --from=fetch --chmod=755 /out/opencode /usr/local/bin/opencode
# The key the core reads for the harness version. Its name is historical (it was the Claude Code
# version's) and it holds the harness version, here OpenCode's; the binary stays the authority.
LABEL candler.cc_version=${OC_VERSION}
USER agent
ENV HOME=/home/agent
WORKDIR /home/agent
