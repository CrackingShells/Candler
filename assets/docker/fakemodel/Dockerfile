# The scripted fake model: serves the Anthropic Messages API over TLS from a script, so the real
# harness runs through the production gateway sidecar without spending a token.
# Base: the python image assets/docker/base pins (it ships the openssl CLI the server uses to
# make its certificate authority at start). Nothing else is fetched.
# Build: docker build --label candler.test=fakemodel -t candler-fakemodel:test assets/docker/fakemodel
FROM python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df
RUN useradd -m -u 10002 -d /home/fake fake
COPY --chmod=0644 fakemodel.py /opt/fakemodel.py
USER fake
WORKDIR /home/fake
EXPOSE 8443
ENTRYPOINT ["python3", "-u", "/opt/fakemodel.py"]
