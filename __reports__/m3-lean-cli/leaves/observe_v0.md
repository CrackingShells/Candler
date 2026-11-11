# observe, leaf report v0

## P5

**Answer: yes, a host-side model-list read.** A preflight can check the gateway's token for free, with no sidecar and no completion request.

- **Request:** `GET <model.endpoint>/v1/models` from the host, with `Authorization: Bearer <value of the secret model.token_secret names>`. It carries no body and asks for no completion.
- **Cost:** a model-list read is a catalogue read, not an inference call, on an OpenAI-style endpoint. The gateway's own readiness probe (`ready_once`, `READY_URL`) makes the same request through the sidecar, and treats 401 as `NotReadyCause::Unauthorized`.
- **Answers seen live,** against the endpoint the repo's smoke experiment names, with the secret that experiment names (`RIVAULT_LLM_INFERENCE`, already set in this shell):

| Request | Status |
|:--|:--|
| no token | 401 |
| the real token | 200 |
| a made-up token | 401 |

- **Reachable from here:** yes, with no VPN step needed at the time of the probe. The unreachable case (no answer, DNS failure, timeout) is read from curl's exit status and treated as "unreachable".
- **What was sent where:** the token went to that one endpoint only, through curl's config on stdin, so it appeared in no argument, log or file. Nothing but the status code was printed. No completion or chat request was made, and no sidecar was started.
- **Consequence:** Step 2 adds `resolve/steps/gateway.rs` (`GatewayStep`, check `gateway_token`).
