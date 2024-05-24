#!/bin/sh
set -e

BINARY=../target/wasm32-unknown-unknown/release/wizdraw_demo_web.wasm
WEB_STATIC=../target/web-static/

cargo +nightly build -r --target wasm32-unknown-unknown
rm -rf $WEB_STATIC
mkdir -p $WEB_STATIC
cp index.html $WEB_STATIC

wasm-bindgen --web $BINARY --out-dir $WEB_STATIC

echo "open http://localhost:8080/index.html"
httpserv $WEB_STATIC
