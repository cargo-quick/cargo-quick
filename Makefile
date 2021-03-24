
.PHONY: fetch-run
fetch-run: 
	(cd fetch && cargo run ~/projects/open-source/rust-repos)

.PHONY: fetch-check
fetch-check: 
	(cd fetch && cargo check)

.PHONY: fetch-clippy
fetch-clippy: 
	(cd fetch && cargo clippy)
