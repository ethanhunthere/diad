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

# The ISO depends on the GUI build: the daemon embeds frontend/dist with
# rust-embed, so the GUI must be built BEFORE the release binary.
iso: gui
	@echo "Building DIAD ISO..."
	@echo "  [1/4] Building diad-agent (release, GUI embedded)..."
	cd $(AGENT_DIR) && cargo build --release
	@echo "  [2/4] Staging diad-agent into the ISO profile..."
	mkdir -p $(ISO_DIR)/airootfs/usr/local/bin
	cp $(TARGET_DIR)/release/$(BINARY) $(ISO_DIR)/airootfs/usr/local/bin/$(BINARY)
	chmod +x $(ISO_DIR)/airootfs/usr/local/bin/$(BINARY)
	@echo "  [3/4] Assembling ISO with mkarchiso (this takes a while)..."
	mkdir -p work out
	sudo mkarchiso -v -w work -o out $(ISO_DIR)
	@echo "  [4/4] Done."
	@echo "ISO built: out/$(ISO_NAME)-*.iso"

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
