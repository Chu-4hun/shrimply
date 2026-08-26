# Shrimply

Shrimply is a simple video editor.

## Development

Run static checks:

```sh
make check
```

Run the app:

```sh
make dev
```

## Documentation

- [Lip sync](docs/lip-sync.md)

## Live MCP server

`make dev` builds `target/debug/shrimply-mcp` alongside the editor. Configure a development
checkout with the absolute binary path:

```toml
[mcp_servers.shrimply]
command = "/absolute/path/to/shrimply/target/debug/shrimply-mcp"
```

An installed release can instead use:

```toml
[mcp_servers.shrimply]
command = "shrimply-mcp"
```

Call `connect_project` with an absolute project path before using other tools or project resources.
The MCP session then connects only to the editor process holding that project lock and can switch
projects by calling `connect_project` again. Resources and tools read and edit the connected
editor's live in-memory project, including unsaved changes. Connection fails clearly when the
project is closed, its lock is stale, or the open editor names another project.
Each MCP client runs its own small stdio adapter. The editor owns one Unix-socket endpoint for its
open project, so different clients can safely reach the same live editor without a global MCP
process or launcher-owned broker.

`view_frame` renders a zero-based project frame with Shrimply's native compositor and returns a PNG
without moving the playhead. Imports without a target choose an existing compatible track with room;
track creation is explicit through `create_track` or `collision = "new_track"`.

After `make dev`, register Shrimply with Codex using:

```sh
make install-codex-mcp-dev
```

## License

Shrimply is licensed under the GNU General Public License, version 3 or later.
See [LICENSE](LICENSE).
