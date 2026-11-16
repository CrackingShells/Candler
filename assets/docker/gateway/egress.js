// The egress proxy's address check. nginx has connected to the resolved upstream when the first
// client byte arrives, and this filter decides on that address before the byte is forwarded: a
// loopback, link-local, unspecified or empty upstream closes the session and nothing is sent.
function refused(s) {
  var addr = String(s.variables.upstream_addr || '');
  var host = addr.charAt(0) === '[' ? addr.slice(1, addr.indexOf(']')) : addr.split(':')[0];
  host = host.toLowerCase();
  var mapped = /^(?:0*:)*:?ffff:(\d+\.\d+\.\d+\.\d+)$/.exec(host);
  if (mapped) {
    host = mapped[1];
  }
  var hex = /^(?:0*:)*:?ffff:([0-9a-f]{1,4}):[0-9a-f]{1,4}$/.exec(host);
  if (hex) {
    var hi = parseInt(hex[1], 16);
    if (hi >> 8 === 127 || hi === 0xa9fe) {
      return true;
    }
  }
  return host === '' || host === '::' || host === '::1' || host === '0.0.0.0'
    || /^127\./.test(host) || /^169\.254\./.test(host) || /^fe[89ab][0-9a-f]:/.test(host);
}

function guard(s) {
  s.on('upload', function (data, flags) {
    if (refused(s)) {
      s.done();
      return;
    }
    s.send(data, flags);
  });
  s.on('download', function (data, flags) {
    if (refused(s)) {
      s.done();
      return;
    }
    s.send(data, flags);
  });
}

export default { guard };
