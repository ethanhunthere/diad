.PHONY: build build-release run test fmt lint check gui iso iso-clean clean all help

# === Agent ===
AGENT_DIR := agent
TARGET_DIR := $(AGENT_DIR)/target
BINARY := diad-agent
FRONTEND_DIR := frontend

build:
	cd $(AGENT_DIR) && cargo build
	@echo "Build complete: $(AGENT_DIR)/target/debug/$(BINARY)"

build-release:
	cd $(AGENT_DIR) && cargo build --release
	@echo "Release build complete: $(AGENT_DIR)/target/release/$(BINARY)"

run:
	cd $(AGENT_DIR) && cargo run

test:
	cd $(AGENT_DIR) && cargo test

fmt:
	cd $(AGENT_DIR) && cargo fmt

lint:
	cd $(AGENT_DIR) && cargo clippy -- -D warnings

check:
	cd $(AGENT_DIR) && cargo check

# === GUI ===
# Install frontend deps and produce frontend/dist (embedded into the binary
# via rust-embed at compile time).
gui:
	@echo "Building DIAD AGENT GUI..."
	@echo "  [1/2] Installing frontend dependencies..."
	cd $(FRONTEND_DIR) && npm ci
	@echo "  [2/2] Building frontend..."
	cd $(FRONTEND_DIR) && npm run build
	@echo "GUI build complete: $(FRONTEND_DIR)/dist"

# === ISO ===
ISO_NAME := diados
ISO_DIR := os/archiso

# F-23 (spec 6.8) — reproducible builds: mkarchiso embeds SOURCE_DATE_EPOCH
# into every timestamped file in the ISO, so `make iso` is byte-reproducible
# for a given source tree. Default to a fixed epoch; override with
# `make iso BUILD_DATE=YYYY-MM-DD` or by exporting SOURCE_DATE_EPOCH yourself.
BUILD_DATE ?= $(shell date -u +%Y-%m-%d)
# -u: resolve BUILD_DATE in UTC so the epoch is identical on every machine.
SOURCE_DATE_EPOCH ?= $(shell date -u -d "$(BUILD_DATE)" +%s 2>/dev/null || date -u +%s)
export SOURCE_DATE_EPOCH

# The ISO depends on the GUI build: the daemon embeds frontend/dist with
# rust-embed, so the GUI must be built BEFORE the release binary.
iso: gui
	@echo "Building DIAD ISO..."
	@echo "  [0/5] Pinned reproducible timestamp SOURCE_DATE_EPOCH=$(SOURCE_DATE_EPOCH)..."
	@echo "  [1/5] Building diad-agent (release, GUI embedded)..."
	cd $(AGENT_DIR) && cargo build --release
	@echo "  [2/5] Staging diad-agent into the ISO profile..."
	mkdir -p $(ISO_DIR)/airootfs/usr/local/bin
	cp $(TARGET_DIR)/release/$(BINARY) $(ISO_DIR)/airootfs/usr/local/bin/$(BINARY)
	chmod +x $(ISO_DIR)/airootfs/usr/local/bin/$(BINARY)
	@echo "  [3/5] Validating nftables rules (build-time gate)..."
	@sudo nft -cf $(ISO_DIR)/airootfs/etc/nftables.conf && echo "  nftables.conf OK" || (echo "  ERROR: nftables.conf failed validation" && exit 1)
	@echo "  [4/5] Assembling ISO with mkarchiso (this takes a while)..."
	mkdir -p work out
	# sudo's env_reset strips SOURCE_DATE_EPOCH, so mkarchiso would fall back
	# to today's date (and the ISO name would drift). Pass it through explicitly.
	sudo env SOURCE_DATE_EPOCH="$(SOURCE_DATE_EPOCH)" mkarchiso -v -w work -o out $(ISO_DIR)
	@echo "  [5/5] Done."
	@echo "ISO built: out/$(ISO_NAME)-$(shell date -u -d @$(SOURCE_DATE_EPOCH) +%Y.%m.%d)-x86_64.iso"

iso-clean:
	sudo rm -rf work out

# === Cleanup ===
clean:
	cd $(AGENT_DIR) && cargo clean
	rm -rf work out
	@echo "Clean complete."

# === Full cycle ===
all: fmt lint build-release test

# === Help ===
help:
	@echo "DIAD Build System"
	@echo ""
	@echo "Available targets:"
	@echo "  build          - Compile debug binary"
	@echo "  build-release  - Compile optimized release binary"
	@echo "  run            - Build and run (debug)"
	@echo "  test           - Run all tests"
	@echo "  fmt            - Format source code"
	@echo "  lint           - Run clippy lints"
	@echo "  check          - Check compilation without building"
	@echo "  gui            - Build the DIAD AGENT GUI (frontend/dist)"
	@echo "  iso            - Build bootable Arch ISO (runs gui first)"
	@echo "  iso-clean      - Remove ISO build artifacts"
	@echo "  clean          - Remove all build artifacts"
	@echo "  all            - Format, lint, release build, and test"
