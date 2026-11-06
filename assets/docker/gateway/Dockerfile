# The gateway sidecar of one job: nginx on the job's internal network. It holds the provider
# credential in a tmpfs file that the supervisor writes through `docker exec -i`, and forwards
# only to one upstream. Nothing in this file or in the image carries the credential.
# Base: nginx 1.28.3 alpine, multi-arch index digest from the gateway spike (02-gateway_spike_v0.md).
# Build: docker build --label candler.test=gateway -t candler-gateway:test assets/docker/gateway
FROM nginx@sha256:a8b39bd9cf0f83869a2162827a0caf6137ddf759d50a171451b335cecc87d236
COPY --chmod=0755 entrypoint.sh /usr/local/bin/gw-entrypoint
COPY nginx.conf.template /etc/gw/nginx.conf.template
COPY gw.js /etc/gw/gw.js
USER nginx
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/gw-entrypoint"]
