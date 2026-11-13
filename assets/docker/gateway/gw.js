// Two checks the nginx config calls. They read the request's own variables: $gw_token and
// $gw_dummy are set by /run/gw/auth.conf, which reaches the sidecar on stdin and nowhere else.

// "1" when the request carries the job's dummy token and no other credential, else "0".
function authentic(r) {
  const dummy = r.variables.gw_dummy;
  if (!dummy) {
    return "0";
  }
  const auth = r.headersIn.Authorization;
  const key = r.headersIn["x-api-key"];
  if (auth === undefined && key === undefined) {
    return "0";
  }
  if (auth !== undefined && auth !== "Bearer " + dummy) {
    return "0";
  }
  if (key !== undefined && key !== dummy) {
    return "0";
  }
  return "1";
}

// The forms in which the token can sit inside another value: as it is, percent-encoded, and in
// base64 at each of the three alignments (without the characters that mix in its neighbours).
function forms(token) {
  const out = [token, encodeURIComponent(token)];
  const bytes = Buffer.from(token);
  for (let pad = 0; pad < 3; pad++) {
    const encoded = Buffer.concat([Buffer.alloc(pad, 0x41), bytes]).toString("base64");
    const drop = pad === 0 ? 0 : pad + 1;
    const middle = encoded.slice(drop, encoded.length - 2);
    if (middle.length >= 8) {
      out.push(middle);
    }
  }
  return out;
}

// Drops every response header whose value holds the token in any of those forms, whatever the
// header is called: the upstream's own words about a credential never reach the agent.
function strip(r) {
  const token = r.variables.gw_token;
  if (!token) {
    return;
  }
  const needles = forms(token);
  const names = Object.keys(r.headersOut);
  for (let i = 0; i < names.length; i++) {
    const value = String(r.headersOut[names[i]]);
    for (let j = 0; j < needles.length; j++) {
      if (value.indexOf(needles[j]) !== -1) {
        delete r.headersOut[names[i]];
        break;
      }
    }
  }
}

// The request's place in its run, counted when the request starts: 1 for the run's first model
// request. The count is per run key and shared by every request of the worker.
function seq(r) {
  return String(ngx.shared.gwseq.incr(r.variables.gw_run, 1, 0));
}

// The name of one entry of a request's `tools` array, in the Anthropic form (`name`) or the
// OpenAI form (`function.name`). Nothing else of the entry is read.
function toolName(entry) {
  if (entry === null || typeof entry !== "object") {
    return undefined;
  }
  if (typeof entry.name === "string") {
    return entry.name;
  }
  if (entry.function !== null && typeof entry.function === "object"
      && typeof entry.function.name === "string") {
    return entry.function.name;
  }
  return undefined;
}

// The census line of an authentic model request: its run, its tag, its sequence number and the
// names of its tools, as a JSON array that is the last field and is escaped by JSON.stringify.
// "" for a request that is not a JSON object or does not carry the job's token. No schema, no
// description and no message text is read, let alone written.
function census(r) {
  if (authentic(r) !== "1") {
    return "";
  }
  let body;
  try {
    body = JSON.parse(r.variables.request_body);
  } catch (e) {
    return "";
  }
  if (body === null || typeof body !== "object" || Array.isArray(body)) {
    return "";
  }
  const tools = Array.isArray(body.tools) ? body.tools : [];
  const names = [];
  for (let i = 0; i < tools.length; i++) {
    const name = toolName(tools[i]);
    if (name !== undefined) {
      names.push(name);
    }
  }
  return "run=" + r.variables.gw_run + " tag=" + r.variables.gw_tag
    + " seq=" + r.variables.gw_n + " tools=" + JSON.stringify(names);
}

export default { authentic, strip, seq, census };
