<p align="center">
  <img src="assets/icons/dev.shrimply.Shrimply.svg" alt="Shrimply logo" width="128">
</p>

# Shrimply

Shrimply brings your ideas to life.

Shrimply is a free and open-source video editor for creating videos from start
to finish, whether you are making a quick edit or something fancy.

Shrimply is currently pre-alpha software.

For more information about Shrimply's features and workflows, visit the
[documentation website](https://shrimply.pages.dev).

## Contributing

Contributions to Shrimply are welcome. There are several ways to help beyond
writing code:

- Report and investigate [issues](https://github.com/soirihiroka/shrimply/issues)
- Improve the documentation
- Test editing workflows and project importers
- Help other users

Before submitting a contribution, read the repository's
[contribution terms](CONTRIBUTING.md).

## Developer Information

### Technology Stack

Shrimply's main application is written in Rust and uses these technologies:

- **Interface**: GTK 4 and libadwaita
- **Rendering**: Skia, wgpu, and CUDA
- **Media**: FFmpeg and PipeWire
- **Compute server**: Python

### Finding Things to Work On

Browse the [open issues](https://github.com/soirihiroka/shrimply/issues) for
reported bugs and planned work. Comment on an issue before starting a larger
change so its scope can be discussed first.

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
