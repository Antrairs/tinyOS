bits 16
org 0x7c00

start:
    xor ax, ax
    mov ds, ax
    mov es, ax

    ; 初始化栈
    cli
    mov ss, ax
    mov sp, 0x7c00
    sti

    ; BIOS 会把启动磁盘号放在 dl
    mov [boot_drive], dl

    ; 从软盘读取下一个扇区

    mov ah, 0x02        ; BIOS: read sectors
    mov al, 2           ; 读取 2 个 sector

    mov ch, 0           ; cylinder 0
    mov dh, 0           ; head 0
    mov cl, 2           ; sector 2

    mov bx, 0x8000      ; 放到内存 0000:8000

    mov dl, [boot_drive]
    int 0x13
    jc disk_error

    ; 跳到 Kernel
    jmp 0x0000:0x8000

disk_error:
    mov si, error_message

.print:
    lodsb
    cmp al, 0
    je halt

    mov ah, 0x0e
    int 0x10

    jmp .print

halt:
    jmp $

boot_drive:
    db 0

error_message:
    db "Disk read error!", 0

times 510 - ($ - $$) db 0
dw 0xaa55