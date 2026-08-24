# OpenVINO Runtime Support

## Goal

Make the published OMZ vision profiles executable on Windows instead of only
downloadable. `office-omz-retail-0288` and `office-omz-retail-0286` must load
their XML/BIN IR pairs through the packaged OpenVINO CPU runtime.

## Design

1. Package the official OpenVINO Windows runtime from the existing release
   workflow's Python environment. The application loads `openvino_c.dll` from
   its Tauri resource directory before creating a `Core`.
2. Keep ONNX Runtime sessions intact. Add a small runtime-neutral session enum
   so the four vision pipeline components can use either ONNX Runtime or
   OpenVINO without changing alert/fusion/storage behavior.
3. Add OMZ-specific preprocessors and decoders:
   - SSD DetectionOutput `[batch, class, confidence, x1, y1, x2, y2]`
   - BGR crops for retail face/person embedding models
   - L2 normalization before existing cosine matching
4. Validate candidate profiles with the backend declared by their V4 manifest,
   including XML/BIN pairing and OpenVINO CPU compilation.
5. Build Windows bundles with `openvino-runtime`; keep macOS on the current
   ONNX-only build until a macOS runtime package is deliberately shipped.

## Verification

- Unit tests for IR-pair validation, packaged runtime lookup, and DetectionOutput decoding.
- `cargo test --lib` on the default feature set.
- `cargo check --features openvino-runtime`.
- Windows release workflow smoke-checks OpenVINO compilation after copying the
  official runtime and before bundling.
