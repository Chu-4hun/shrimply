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

- Build the documentation site with `make docs`.
- Open `docs/build/index.html` after the build.
- The source lives under [`docs/source`](docs/source).

MCP setup, tools, resources, and connection behavior are documented in the
[MCP integration guide](docs/source/integrations/mcp.rst).

## License

Shrimply is licensed under the GNU General Public License, version 3 or later.
See [LICENSE](LICENSE).

Shrimply itself is free software, but some features depend on components that
are not free software. These include NVIDIA's [CUDA Toolkit and display
driver](https://docs.nvidia.com/cuda/eula/), [OptiX
SDK](https://developer.nvidia.com/designworks/optix/download), [Optical Flow
SDK](https://developer.nvidia.com/optical-flow-sdk), and [Video Codec
SDK](https://developer.nvidia.com/video-codec-sdk), as well as separately
licensed model weights. Those components retain their own license terms; see
the [license documentation](docs/source/licenses.rst) and
[third-party notices](THIRDPARTY.md) for details.
