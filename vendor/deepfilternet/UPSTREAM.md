# Vendored DeepFilterNet

This directory contains the Rust inference core from
[`Rikorose/DeepFilterNet`](https://github.com/Rikorose/DeepFilterNet) commit
`d375b2d8309e0935d165700c91da9de862a99c31` and the low-latency
DeepFilterNet3 ONNX model used by Shrimply.

Python packages, training and dataset code, command-line tools, alternate
models, C bindings, WASM bindings, and file-format helpers are intentionally
excluded. The Rust dependencies are maintained by Shrimply so the inference
core can use the current tract and ndarray releases.
