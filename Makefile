KERNEL = target/riscv64gc-unknown-none-elf/debug/xv6-riscv-rust
CPUS = 3

QEMU = qemu-system-riscv64
QEMUOPTS = -machine virt -bios none -kernel $(KERNEL) -m 3G -smp $(CPUS) -nographic
QEMUGDB = -gdb tcp::26000

OBJDUMP = riscv64-unknown-elf-objdump

qemu-gdb:
	cargo build
	@echo "*** Now run 'gdb' in another window." 1>&2
	$(QEMU) $(QEMUOPTS) -S $(QEMUGDB)

asm:
	cargo build
	$(OBJDUMP) -S $(KERNEL) > kernel.S

clean:
	rm -rf kernel.S
	cargo clean
