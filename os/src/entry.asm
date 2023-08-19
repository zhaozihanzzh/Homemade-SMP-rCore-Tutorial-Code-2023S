    .section .text.entry
    .globl _start
_start:
    la sp, boot_stack_top
    slli a2, a0, 13 # multiply 8192
    sub sp, sp, a2 # allocate 2*4KiB
    mv tp, a0
    call rust_main

    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16
    .globl boot_stack_top
boot_stack_top:

    .section .text.entry
    .globl _start_backup_hart
_start_backup_hart:
    la sp, boot_stack_top
    slli a2, a1, 13 # multiply 8192
    sub sp, sp, a2 # allocate 2*4KiB
    mv tp, a1
    call start_backup_hart