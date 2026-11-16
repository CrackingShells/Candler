# The extension of the `tiny` environment: one dependency from a vendored wheel, installed
# with no package index.
ARG BASE
FROM ${BASE}
USER root
COPY wheels /tmp/wheels
RUN uv pip install --python /opt/env/bin/python --no-index --no-build --find-links /tmp/wheels tinydep \
 && rm -rf /tmp/wheels \
 && chmod -R a+rwX /opt/env
USER agent
