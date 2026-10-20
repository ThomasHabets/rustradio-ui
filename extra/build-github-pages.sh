#!/usr/bin/env bash
set -ueo pipefail
DIST_DIR="$(pwd)/$(dirname "$0")/../dist"
echo "=> $DIST_DIR"

export GIT="$(git describe --tags --dirty --always)"

mkdir -p "$DIST_DIR/uhd"
cp assets/examples/* "$DIST_DIR/"
sed "s/GIT_VERSION_NOT_SET/${GIT}/g" \
        assets/examples/index.html \
        > "$DIST_DIR/index.html"
(
        cd "$UHD_IMAGES_DIR"
        cp \
                usrp_b200_fw.hex \
                usrp_b200_fpga.bin \
                "$DIST_DIR/uhd/"
)
for example in iq-waterfall rtlsdr-fm; do
        (
                cd "examples/$example"
                wasm-pack build \
                        --target web \
                        --out-dir "$DIST_DIR/$example" \
                        "--${1:-profiling}"
                cp web/* "$DIST_DIR/$example/"
                cp ../../assets/bootstrap.js "$DIST_DIR/$example/rustradio-ui-bootstrap.js"
                cat \
                        ../../assets/rustradio.css \
                        web/style.css \
                        > "$DIST_DIR/$example/style.css"
                sed "s/GIT_VERSION_NOT_SET/${GIT}/g" \
                        web/index.html \
                        > "$DIST_DIR/$example/index.html"
                for f in "$DIST_DIR/uhd/"*; do
                        ln -f -s "$f" "$DIST_DIR/$example/"
                done
        )
done
