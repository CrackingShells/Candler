import os, subprocess, sys


def argv_of(pid):
    try:
        with open("/proc/" + str(pid) + "/cmdline", "rb") as handle:
            return [a.decode("utf-8", "replace") for a in handle.read().split(b"\0")[:-1]]
    except OSError:
        return []


def parent_of(pid):
    # Field 4 of /proc/<pid>/stat, after the ")" that ends the command name.
    try:
        with open("/proc/" + str(pid) + "/stat") as handle:
            text = handle.read()
        return int(text[text.rindex(")") + 2:].split()[1])
    except (OSError, ValueError, IndexError):
        return 0


def is_server(argv, parent_argv):
    # The server child of `opencode run --standalone`: `<path>/opencode serve --stdio --port 0`,
    # started by the `opencode run` client. Matched by whole argument vectors, never by a
    # substring of a command line.
    server = len(argv) == 5 and os.path.basename(argv[0]) == "opencode" and argv[1:] == ["serve", "--stdio", "--port", "0"]
    client = len(parent_argv) > 1 and os.path.basename(parent_argv[0]) == "opencode" and parent_argv[1] == "run"
    return server and client


def main():
    pid = None
    for name in os.listdir("/proc"):
        if name.isdigit() and is_server(argv_of(name), argv_of(parent_of(name))):
            pid = name
    if pid is None:
        sys.exit(3)

    # The child makes its own password and listens on a random loopback port: read both from /proc.
    with open("/proc/" + pid + "/environ", "rb") as handle:
        pairs = [item.split(b"=", 1) for item in handle.read().split(b"\0") if b"=" in item]
    password = dict((key, value) for key, value in pairs).get(b"OPENCODE_PASSWORD")
    if password is None:
        sys.exit(4)
    sockets = set()
    for fd in os.listdir("/proc/" + pid + "/fd"):
        try:
            link = os.readlink("/proc/" + pid + "/fd/" + fd)
        except OSError:
            continue
        if link.startswith("socket:["):
            sockets.add(link[8:-1])
    host = port = None
    for table in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            with open(table) as rows:
                lines = list(rows)[1:]
        except OSError:
            continue
        for row in lines:
            cols = row.split()
            if cols[3] == "0A" and cols[9] in sockets:
                address, number = cols[1].split(":")
                port = int(number, 16)
                host = "[::1]" if address == "00000000000000000000000001000000" else "127.0.0.1"
    if port is None:
        sys.exit(5)

    env = dict(os.environ)
    env["OPENCODE_PASSWORD"] = password.decode("utf-8", "replace")
    done = subprocess.run(
        ["opencode", "api", "--server", "http://" + host + ":" + str(port), "session.interrupt", "--param", "sessionID=" + sys.argv[1]],
        env=env,
    )
    sys.exit(done.returncode)


if __name__ == "__main__":
    main()
