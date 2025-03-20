.PHONY: publish
publish:
	cargo package
	cargo publish

.PHONY: test
test:
	cargo test -- --nocapture
