.PHONY: build frontend backend clean install uninstall

# Build everything — frontend first (rust-embed needs the dist at compile time).
build: frontend backend

# Build the Vue frontend (output to dist_new, read by rust-embed at compile time).
frontend:
	cd frontend && npx vite build --outDir dist_new --emptyOutDir

# Build the Rust backend in release mode.
backend:
	cd backend && cargo build --release

# Install the binary to /usr/local/bin.
install: build
	cp backend/target/release/obsidian-brain /usr/local/bin/
	@echo "Installed: obsidian-brain"
	@echo "Run 'obsidian-brain start' to start the server."

# Remove the installed binary.
uninstall:
	rm -f /usr/local/bin/obsidian-brain
	@echo "Uninstalled."

# Clean build artifacts.
clean:
	rm -rf frontend/dist frontend/dist_new
	cd backend && cargo clean
