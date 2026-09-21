# The sandbox this project is developed and tested in.
#
# Why it exists: the tool quarantines files, rewrites git history and walks
# $HOME. Running its test suite directly on a workstation means one bad path
# expansion away from moving somebody's real files. Every test run goes through
# polinrider-sandbox, which uses this image and gives the run a $HOME of its own.
#
# Pinned by digest, not by tag. A supply-chain cleanup tool that pulls a
# floating base image is not making an argument it can defend.
FROM rust:1.98.1-slim-bookworm@sha256:ff521445a372125ed4f76e1453a1f8098f2d05332d1601d30db1c1f62757e730 AS base

# bash       the scripts under test; the image's default sh is dash
# git        the repository tracks and the fixtures
# jq         the conformance runner parses cases with it
# shellcheck lint, so the sandbox can run the same checks CI does
# curl, ca-certificates  fetching indicator sources
# procps     ps, which the live-host checks read
RUN apt-get update && apt-get install -y --no-install-recommends \
      bash git jq shellcheck curl ca-certificates procps \
 && rm -rf /var/lib/apt/lists/*

# mdbook is deliberately NOT here. The sandbox exists to contain destructive
# filesystem work; rendering Markdown is not that, and the release tarball is
# per-architecture, which broke the build on arm64 with a SIGTRAP. Docs build
# on the host and in the docs workflow.

# gh, a declared dependency of the GitHub recovery tracks. Several self-tests
# exercise those paths entirely offline, but the dependency check still runs.
# Fetched per-architecture: the mdbook tarball was x86_64-only and died with a
# SIGTRAP on arm64, which is a mistake worth making once.
ARG TARGETARCH
ARG GH_VERSION=2.101.0
RUN set -eux; \
    case "${TARGETARCH:-amd64}" in \
      amd64) gharch=amd64 ;; \
      arm64) gharch=arm64 ;; \
      *) echo "unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    curl -sSL "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${gharch}.tar.gz" \
      -o /tmp/gh.tgz; \
    tar -xzf /tmp/gh.tgz -C /tmp; \
    install -m0755 "/tmp/gh_${GH_VERSION}_linux_${gharch}/bin/gh" /usr/local/bin/gh; \
    rm -rf /tmp/gh.tgz "/tmp/gh_${GH_VERSION}_linux_${gharch}"; \
    gh --version

# Never root. A test that escapes its temp directory should hit a permission
# error, not a writable filesystem.
ARG UID=1000
ARG GID=1000
RUN groupadd -g "${GID}" dev 2>/dev/null || true \
 && useradd -m -u "${UID}" -g "${GID}" -s /bin/bash dev 2>/dev/null || true
USER dev
ENV HOME=/home/dev
WORKDIR /work

# A UTF-8 locale. Without one the banner width check counts bytes, not
# characters, and reports a 75-column wordmark as 207.
# LANG only. Setting LC_ALL here would override every other locale variable
# and could not be undone by a test that deliberately simulates a non-UTF-8
# terminal, which is a case this tool has to handle.
ENV LANG=C.UTF-8

# An identity, because several self-tests build real repositories and commit to
# them. These values are never pushed anywhere; they exist so git will run.
#
# Deliberately NOT setting init.defaultBranch: fixtures choose their own branch
# names, and forcing main here made a restore test look for refs/heads/master
# in a repository that had none, which reads as 'a dry run modified the mirror'.
RUN git config --global user.email "sandbox@localhost" \
 && git config --global user.name  "polinrider sandbox" \
 && git config --global --add safe.directory /work

# cargo writes here; /work is mounted read-only by polinrider-sandbox.
# This lives under a single subdirectory on purpose: polinrider-sandbox mounts a
# named volume here, and a volume mounted at $HOME itself would mask the
# .gitconfig written above, along with anything else the image puts in home.
RUN mkdir -p /home/dev/build
ENV CARGO_TARGET_DIR=/home/dev/build/target
ENV CARGO_HOME=/home/dev/build/cargo

SHELL ["/bin/bash", "-c"]
CMD ["bash"]
