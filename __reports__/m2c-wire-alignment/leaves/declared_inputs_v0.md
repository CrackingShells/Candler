# declared_inputs leaf: report v0 (in progress)

## P2

Question: does docker take one read-only bind mount per file (40 files in nested directories), with the parent workspace directories still writable, under the run container's flags (`--init --read-only`, tmpfs `/tmp`, `--cap-drop ALL`, `no-new-privileges`, the workspace as a read-write bind mount at `/work`)?

Evidence (Docker Desktop on macOS, `ubuntu:24.04`, 40 files in 5 x 3 nested directories, 40 `type=bind,readonly` mounts under a `/work` bind mount):

| trial | result |
|:------|:-------|
| mount points not made in the host workspace | the daemon refuses at container creation: `create mountpoint for /work/d0/s0/f15.txt mount: mountpoint ... is outside of rootfs` (one file is enough) |
| an empty file made at each mount point in the host workspace first | the container starts; 40 of 40 files read with their content |
| append to a mounted file | `Read-only file system` |
| move a mounted file | `Device or resource busy` (the mount point cannot be renamed away) |
| new file beside a mounted one, new directory with a file in it | both succeed |
| host side after the run | source files unchanged; the workspace holds the 40 empty stubs plus the new file |

Decision: one read-only file mount per file, with the mount point made first by the host user as an empty file (and its parent directories) in the workspace. This is what `mount_inputs` already does, so its form stays; the stubs are required, not a convenience. A materialised read-only directory mount is not needed.

Containers created by the probe, all removed by exact id: `23855e7ace73` (failed create, removed), `4965ab8299e0`, `1fcf09540b76` (created, never started), `4317593f1dd7`. The throwaway tree was removed.
