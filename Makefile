.PHONY: build frontend backend clean install uninstall

# Build everything — frontend first (rust-embed needs the dist at compile time).
build: frontend backend

# Build the Vue frontend and copy dist into backend/ for rust-embed.
frontend:
	cd frontend && npm run build
	rm -rf backend/frontend/dist
	mkdir -p backend/frontend/dist
	cp -r frontend/dist/* backend/frontend/dist/

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
	rm -rf frontend/dist backend/frontend/dist
	cd backend && cargo clean
