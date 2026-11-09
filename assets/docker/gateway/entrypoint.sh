#!/bin/sh
# Waits for the credential file, renders the config and runs nginx in the foreground.
# Until /run/gw/auth.conf exists nothing listens, so the sidecar serves no upstream request.
# The supervisor writes the file through auth.tmp and mv, so nginx never reads a partial file.
set -eu

waited=0
while [ ! -s /run/gw/auth.conf ]; do
  waited=$((waited + 1))
  if [ "$waited" -gt 600 ]; then
    echo "gateway: no credential file after 60 s" >&2
    exit 1
  fi
  sleep 0.1
done

# The upstream is named by plain, non-secret variables the supervisor passed at start.
: "${GW_UPSTREAM_HOST:?}" "${GW_UPSTREAM_AUTHORITY:?}" "${GW_MAX_PARALLEL:?}"
if [ -s /run/gw/ca.pem ]; then
  GW_CA=/run/gw/ca.pem
else
  GW_CA=/etc/ssl/certs/ca-certificates.crt
fi
export GW_CA

envsubst '${GW_UPSTREAM_HOST} ${GW_UPSTREAM_AUTHORITY} ${GW_MAX_PARALLEL} ${GW_CA}' \
  < /etc/gw/nginx.conf.template > /tmp/nginx.conf
exec nginx -c /tmp/nginx.conf -g 'daemon off;'
