#!/bin/bash
cargo run --package frameever-recording --example synthetic-test-runner --features test-utils -- "$@"
