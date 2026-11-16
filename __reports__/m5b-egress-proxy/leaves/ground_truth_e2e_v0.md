# ground_truth_e2e leaf report (v0) - PARTIAL

Findings. Step 1 done; step 2 (the e2e `i8_canary_never_reaches_the_agent`) not built, for the reasons under Findings.

1. Commits: c39e76cb `test(isolation): add a canary seeker mode to the testagent`.
2. Gates: step 1 check PASS (`sh -n` clean, `seeker` in script header). Step 2 gate, SPAWN, STD, LINUX, E2E, SUBJ: not run (no step 2 code; machine load average 76 at start, brief limit 12).
3. Guards: none written. Seeker verified once by hand in a busybox:1.37.0 container (script mounted, no image built): a canary file under a ro mount, and a canary env var, were reported at start and end as `seeker:<phase>:file:`, `environ:<pid>:`, `mount:` lines, then `done`.
4. Probes: none owned.
5. Claims checked (colgrep):
   - "testagent base serves the plug-in environment": the binary has no setting or seam that runs the testagent image as the agent. `assets.rs` only embeds its build context. The base image is the Claude Code image (python:3.12-slim + CC_VERSION, user 10001); the testagent is busybox. `candler run` launches the harness's `claude` argv in the env image over that base.
   - The cli rig (`support/cli_run.rs`) is a PATH-fake docker with no real container. The only real-daemon cli e2e (`guard_cli_e2e.rs`) uses a forwarding shim and starts testagent containers by hand, not through `candler run`.
6. Findings:
   - SPEC DEFECT (blocking step 2): the leaf asks for a seeker inside the agent container of a real `candler run`, but the agent there is the real Claude Code binary, not the testagent. Options need a ruling: (a) a fixture env Dockerfile (`isolation.build.dockerfile` is a request setting) that puts a `claude` stand-in script, built on the cached `candler-base`, in the image, which would also need the harness's version probe and stream-json shape satisfied; or (b) the fake model scripts a Bash tool call that runs the seeker inside real Claude Code, which needs the fakemodel, gateway and env images built (Claude Code 2.1.304 image cached as youzuoqi/claude-code). Both are src-free but large; neither fits 45 calls under the colgrep-only reading rule.
   - The seeker prints on stdout. Under (a) that reaches the trace as frames only if the stand-in wraps it in stream-json.
   - No `src/` file or other out-of-OWNED path edited. No behaviour change.
7. Docker: images 14 / containers 3 / networks 5 before, identical after (only a `--rm` busybox run; busybox:1.37.0 was already present). No images pulled or built. Container ids before and after: a74248eec16c a8f731202eb1 d4bc746de8bb. Scratch `~/m5b-seek` removed; the target dir `/private/tmp/claude-501/m5b-target-ground_truth_e2e` holds only a commit message file. e2e name reserved: `guard_cli_e2e::i8_e2e::i8_canary_never_reaches_the_agent` (not built).
