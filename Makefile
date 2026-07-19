# Makefile for Git HTTP Router

# Variables
BINARY_NAME=git-http-router
TARGET_MUSL_X86=x86_64-unknown-linux-musl
TARGET_MUSL_ARM=aarch64-unknown-linux-musl
CARGO ?= cargo

.PHONY: build build-musl-amd64 build-musl-arm64 run test fmt clean install

# Standard development build
build:
	$(CARGO) build
	mkdir -p bin
	cp target/debug/$(BINARY_NAME) bin/$(BINARY_NAME)

# Statically linked MUSL build for Linux (x86_64)
build-musl-amd64:
	$(CARGO) build --release --target $(TARGET_MUSL_X86)
	mkdir -p bin
	cp target/$(TARGET_MUSL_X86)/release/$(BINARY_NAME) bin/$(BINARY_NAME)-amd64

# Statically linked MUSL build for Linux (ARM64)
build-musl-arm64:
	$(CARGO) build --release --target $(TARGET_MUSL_ARM)
	mkdir -p bin
	cp target/$(TARGET_MUSL_ARM)/release/$(BINARY_NAME) bin/$(BINARY_NAME)-arm64

# Run locally
run:
	$(CARGO) run

# Run tests
test:
	$(CARGO) test

# Format the code
fmt:
	$(CARGO) fmt

# Clean the target directory
clean:
	$(CARGO) clean
	rm -rf bin/

# Install locally via cargo
install:
	$(CARGO) install --path .
