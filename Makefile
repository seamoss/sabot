# Sabot Makefile
# Usage: make [target]

BINARY    := sabot
CARGO     := cargo
VERSION   := $(shell cat VERSION 2>/dev/null | tr -d '[:space:]')

# Default: build in debug mode
.PHONY: all
all: build

# ============================================================
# Build
# ============================================================

.PHONY: build
build: ## Build in debug mode
	$(CARGO) build

.PHONY: release
release: ## Build in release mode
	$(CARGO) build --release

.PHONY: clean
clean: ## Remove build artifacts
	$(CARGO) clean

# ============================================================
# Test
# ============================================================

.PHONY: test
test: build ## Run all tests (Rust + Sabot)
	$(CARGO) test
	./target/debug/$(BINARY) test lib/

.PHONY: test-rust
test-rust: ## Run Rust unit tests only
	$(CARGO) test

.PHONY: test-sabot
test-sabot: build ## Run Sabot test suite only
	./target/debug/$(BINARY) test lib/

# ============================================================
# Lint & Format
# ============================================================

.PHONY: fmt
fmt: ## Format Rust source code
	$(CARGO) fmt

.PHONY: fmt-check
fmt-check: ## Check Rust formatting (CI mode)
	$(CARGO) fmt --check

.PHONY: clippy
clippy: ## Run Clippy lints
	$(CARGO) clippy -- -D warnings

.PHONY: lint
lint: fmt-check clippy ## Run all lints

.PHONY: fmt-sabot
fmt-sabot: build ## Format all Sabot source files in place
	@for f in lib/*.sabot examples/*.sabot programs/*.sabot games/*.sabot tools/*.sabot; do \
		[ -f "$$f" ] && ./target/debug/$(BINARY) fmt -w "$$f"; \
	done

# ============================================================
# Tools
# ============================================================

.PHONY: bench
bench: release ## Run benchmarks
	./target/release/$(BINARY) tools/bench.sabot

.PHONY: profile
profile: build ## Profile (usage: make profile FILE=examples/hello.sabot)
	./target/debug/$(BINARY) profile $(FILE)

.PHONY: repl
repl: build ## Start the REPL
	./target/debug/$(BINARY)

# ============================================================
# Version Management
# ============================================================
#
# VERSION file is the single source of truth.
# scripts/version-sync.sh propagates it to all other files.

.PHONY: version
version: ## Show current version
	@echo "$(VERSION)"

.PHONY: version-check
version-check: ## Verify all files match VERSION (for CI)
	@./scripts/version-sync.sh --check

.PHONY: bump
bump: ## Bump version: make bump V=0.5.0
ifndef V
	$(error Usage: make bump V=x.y.z)
endif
	@./scripts/version-sync.sh $(V)
	@echo ""
	@echo "Next steps:"
	@echo "  1. Update CHANGELOG.md with release notes"
	@echo "  2. git add -A && git commit -m 'chore: bump version to v$(V)'"
	@echo "  3. git push origin dev"
	@echo "  4. Open PR: dev -> main"

# ============================================================
# Release Pipeline
# ============================================================

.PHONY: check
check: lint version-check test ## Full pre-release check (lint + version + test)

.PHONY: release-dry-run
release-dry-run: check ## Simulate a release (run all checks, show what would happen)
	@echo ""
	@echo "=== Release Dry Run ==="
	@echo "Version:  $(VERSION)"
	@echo "Binary:   $(BINARY)"
	@echo "Tag:      v$(VERSION)"
	@echo ""
	@echo "All checks passed. Ready to release."
	@echo ""
	@echo "Workflow:"
	@echo "  1. make bump V=x.y.z"
	@echo "  2. Update CHANGELOG.md"
	@echo "  3. git commit && git push origin dev"
	@echo "  4. Merge dev -> main (PR or direct)"
	@echo "  5. Auto-tag + release triggered on main"

# ============================================================
# Local CI (act — run GitHub Actions locally via Docker)
# ============================================================
#
# Requires: brew install act, Docker running
# Config:   .actrc (runner image, reuse containers)
# Events:   .github/act-events/*.json

ACT := act
ACT_EVENTS := .github/act-events

.PHONY: act-ci
act-ci: ## Run full CI workflow locally
	$(ACT) push -e $(ACT_EVENTS)/push-main.json -W .github/workflows/ci.yml

.PHONY: act-ci-lint
act-ci-lint: ## Run just the lint + fmt jobs from CI
	$(ACT) push -e $(ACT_EVENTS)/push-main.json -W .github/workflows/ci.yml -j fmt-check -j clippy

.PHONY: act-ci-test
act-ci-test: ## Run just the build-and-test job from CI
	$(ACT) push -e $(ACT_EVENTS)/push-main.json -W .github/workflows/ci.yml -j build-and-test

.PHONY: act-ci-version
act-ci-version: ## Run just the version-check job from CI
	$(ACT) push -e $(ACT_EVENTS)/push-main.json -W .github/workflows/ci.yml -j version-check

.PHONY: act-tag
act-tag: ## Run auto-tag workflow locally (dry run — won't push)
	$(ACT) push -e $(ACT_EVENTS)/push-main.json -W .github/workflows/auto-tag.yml

.PHONY: act-release
act-release: ## Run release workflow locally (Linux x86 only)
	$(ACT) push -e $(ACT_EVENTS)/push-tag.json -W .github/workflows/release.yml

.PHONY: act-list
act-list: ## List all jobs act would run
	$(ACT) -l -W .github/workflows/

# ============================================================
# Install
# ============================================================

.PHONY: install
install: ## Install sabot to ~/.cargo/bin
	$(CARGO) install --path .

.PHONY: uninstall
uninstall: ## Uninstall sabot from ~/.cargo/bin
	$(CARGO) uninstall $(BINARY)

# ============================================================
# Help
# ============================================================

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'
