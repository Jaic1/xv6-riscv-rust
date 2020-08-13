KERNEL = kernel
CPUS = 3

QEMU = qemu-system-riscv64
QEMUOPTS = -machine virt -bios none -m 3G -smp $(CPUS) -nographic
QEMUOPTS += -drive file=fs.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0
QEMUGDB = -gdb tcp::26000

OBJDUMP = riscv64-unknown-elf-objdump

qemu-gdb:
	cargo build
	@echo "*** Now run 'gdb' in another window." 1>&2
	$(QEMU) $(QEMUOPTS) -kernel $(KERNEL) -S $(QEMUGDB)

qemu:
	$(QEMU) $(QEMUOPTS) -kernel $(KERNEL)

qemu-syscall:
	$(QEMU) $(QEMUOPTS) -kernel $(KERNEL)_syscall

asm:
	cargo build
	$(OBJDUMP) -S $(KERNEL) > kernel.S

clean:
	rm -rf kernel.S
	cargo clean
