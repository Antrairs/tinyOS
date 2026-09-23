bits 16
org 0x8000

start:
    ; 引导程序在实模式下跳到 0000:8000
    cli
    xor ax, ax
    mov ss, ax
    mov sp, 0x7c00
    mov ds, ax
    mov es, ax
    cld
    sti

    mov si, message
    call print

input_loop:
    mov ah, 0
    int 0x16
    cmp al, 13
    je halt
    mov ah, 0x0e
    int 0x10
    jmp input_loop

halt:
    hlt
    jmp halt

print:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0e           ; BIOS teletype output
    mov bx, 0x0007         ; page 0, light gray
    int 0x10
    jmp print
.done:
    ret

message:
    db "Hello OS!", 13, 10

times 1024 - ($ - $$) db 0