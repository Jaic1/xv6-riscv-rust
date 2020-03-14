OUTPUT_ARCH("riscv")
ENTRY(_entry)

SECTIONS
{
  /*
   * ensure that entry.S / _entry is at 0x80000000,
   * where qemu's -kernel jumps.
   */
  . = 0x80000000;
  .text :
  {
    *(.text)
  }

  .rodata :
  {
    *(.rodata)
  }

  /*
   * make sure end is after data and bss.
   */
  .data : {
    *(.data)
  }
  .bss : {
    *(.bss)
    *(.sbss*)
  }
  PROVIDE(end = .);
}
