# The environment layer: one venv at /opt/env built from the resolved lock, on the pinned base.
# BASE is `candler-base:<id12>@sha256:<base image id>`; the digest pins the base and a wrong
# digest fails the build. It has no default.
ARG BASE
FROM ${BASE}
USER root
ENV UV_LINK_MODE=copy UV_PYTHON_DOWNLOADS=never UV_NO_CACHE=1 PYTHONDONTWRITEBYTECODE=1
RUN uv venv /opt/env --seed --python 3.12
# --no-build makes an sdist a build failure, so nothing compiles.
RUN --mount=type=bind,source=requirements.lock,target=/tmp/requirements.lock \
    uv pip install --python /opt/env/bin/python --no-build -r /tmp/requirements.lock \
 && find /opt/env -type f \( -name domain.py -o -name problems.py \) -delete \
 && chmod -R a+rwX /opt/env
USER agent
ENV VIRTUAL_ENV=/opt/env UV_PROJECT_ENVIRONMENT=/opt/env PATH=/opt/env/bin:$PATH
