build:
	$(MAKE) -C sim/contracts build
	cargo build

clean:
	$(MAKE) -C sim/contracts clean
	rm -rf target
