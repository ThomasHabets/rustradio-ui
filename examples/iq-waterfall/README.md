# I/Q waterfall

A small WASM client for a native `IqStreamSink`. Enter a host and port, click
**Fetch streams**, then select a source and click **Start streaming** to view a
waterfall. It uses the experimental
`rustradio-ui::worker::IqStreamSource`, with socket and DSP work in a worker.
There is no WebUSB or audio setup.

## Build and run

Install `wasm-pack` and the nightly Rust toolchain with `rust-src`, then run:

```sh
cd examples/iq-waterfall
./build-local.sh
python3 ../serve.py
```

Open <http://127.0.0.1:8080>. The included server sets the COOP and COEP headers
needed by the shared-memory worker. Build output is in `web-dist/`.
`./build-local.sh release` builds an optimized version.

The packaging script embeds the same Git version in the HTML and WASM. Startup
checks these versions before enabling controls or starting the worker. Missing
or mismatched versions produce a visible error; rebuild the package or reload
and clear cached assets if an old version is being served. Direct WASM builds
alone do not replace the HTML version placeholder; use `build-local.sh` to
produce a complete package.

For a test stream, run this from the `rustradio` repository root:

```sh
cargo run --example iq_waterfall
```

Use host `127.0.0.1`, port `50051`, select `iq`, and **Blocking (lossless)**.
The test server generates a complex tone at +6 kHz with a 48 ksample/s rate.
Use **Stop streaming** and **Start streaming** to stop and restart. Stop the
server with Ctrl-C.

## Stream settings

- Enter the host and port, then click **Fetch streams** to populate the source
  dropdown. Select a source and click **Start streaming**. No server requests
  happen until you click Fetch streams. Changing the host, port, or TLS setting
  clears the list so you can fetch from the new address. Non-complex,
  unsupported, and ended streams stay visible but cannot be selected.
- This viewer accepts **complex float32** samples. The protocol supplies the
  sample rate; the waterfall axis updates automatically after connection.
  Frequencies are relative to the stream center, from −rate/2 to +rate/2.
- Choose the mode matching the sink's `.blocking(bool)`: lossless for `true`,
  allow gaps for `false`. A mismatch is reported by the server.
- TLS selects `wss://`; otherwise the URL is `ws://HOST:PORT/iq/v1/stream`.
  HTTPS pages select TLS by default. For remote servers, use TLS, which can be
  provided by a reverse proxy in front of the native listener. For a plain HTTP
  server on `127.0.0.1`, uncheck TLS and follow the local-access notes below.
- Discovery uses the HTTP representation of `ListStreams` at
  `http(s)://HOST:PORT/iq/v1/streams`. If the server has a different origin from
  this page, its catalog response must include `Access-Control-Allow-Origin`
  permitting the page (or `*`). Use a server with the catalog CORS fix or configure
  these headers in the reverse proxy; rustradio 0.19.4 does not include them.
  A failed or empty discovery leaves Start streaming disabled; click Fetch streams
  to try again.

## GitHub Pages with a local server

Use host `127.0.0.1` and your server's port. Uncheck **Use a secure connection
(TLS)** if the server speaks plain HTTP, then click **Fetch streams**. In
browsers supporting local network access permissions, allow the Pages site to
access the loopback/local network when prompted. If previously denied, change
that permission in the browser's site settings and try again. The server still
needs the catalog CORS headers described above.

The isolation service worker handles only same-origin application files. Local
catalog requests go directly from the page so the browser can ask for network
permission. Changing COOP/COEP headers does not grant this permission.

Browser support for HTTP/WS connections from an HTTPS page varies. If your
browser blocks them, serve the viewer locally using the instructions above, or
provide trusted HTTPS/WSS on the streaming server.

The graph uses 2048-sample Hamming windows and computes at most about 30 FFT
rows per second. Display queues are bounded and may skip rows if rendering is
slow. FFT windows containing a stream gap are discarded, so a row never mixes
samples from both sides of a discontinuity. The final status reports missing
samples; it does not show gaps as elapsed-time rows. A final partial FFT window
is discarded. This example has no automatic reconnection or recording.
