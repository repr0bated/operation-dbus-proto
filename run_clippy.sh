#!/bin/bash
set -x
source $HOME/.cargo/env || true
export PATH="$HOME/.cargo/bin:$PATH"

cargo --version
rustc --version
cargo clippy --message-format=json > clippy_output.json 2> clippy_error.log
echo "FINISHED_CODE: $?" > clippy_done.txt
