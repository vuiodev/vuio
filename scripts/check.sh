#!/bin/bash
cargo fmt -p vuio-core -p vuio-cli -p vuio-cast -p vuio-web -- --check
cargo clippy -p vuio-core -p vuio-cli -p vuio-cast -p vuio-web --all-targets --all-features -- -D warnings