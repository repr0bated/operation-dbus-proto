#!/bin/bash
cargo clippy --message-format=json > clippy_output.json 2> clippy_error.log
echo "FINISHED" > clippy_done.txt
