# Candler M2b test agent (scaffold S3, fixture owner: the lead). Not candler-base.
# Build once as: docker build --label candler.test=1 -t candler-testagent:test assets/docker/testagent
# Base: busybox 1.37.0, multi-arch index digest recorded 2026-11-06 from `docker pull busybox:1.37.0`.
FROM busybox@sha256:a7dbc319b1438115a207e11930dc4bb3e643dcb3ec0bdd694bb36cd8fad289ec
RUN addgroup -g 10001 agent && adduser -D -u 10001 -G agent -h /home/agent agent \
 && mkdir -p /workspace && chown agent:agent /workspace /home/agent
COPY --chmod=0755 testagent.sh /usr/local/bin/testagent
USER 10001:10001
ENV HOME=/home/agent
WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/testagent"]
