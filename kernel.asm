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

command_loop:
    mov si, prompt
    call print
    mov di, input_buffer

input_loop:
    mov ah, 0
    int 0x16
    cmp al, 13
    je finish_input
    cmp di, input_buffer + 15
    jae input_loop    ; DI 已到最后一个字节：不保存，也不显示
    mov [di], al      ; 保存当前字符
    inc di            ; 指向下一个空位置
    mov ah, 0x0e
    int 0x10
    jmp input_loop

finish_input:
    ; 回车换行
    mov ah, 0x0e
    mov al, 13 ; CR 回到当前行行首
    int 0x10
    mov al, 10 ; LF 下移一行
    int 0x10

    mov byte [di], 0

    ; 检查 help 命令
    cmp byte [input_buffer], 'h'
    jne unknown_command
    cmp byte [input_buffer + 1], 'e'
    jne unknown_command
    cmp byte [input_buffer + 2], 'l'
    jne unknown_command
    cmp byte [input_buffer + 3], 'p'
    jne unknown_command
    cmp byte [input_buffer + 4], 0
    jne unknown_command

    jmp help_command

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

prompt:
    db "tinyOS> ", 0

message:
    db "Hello OS!", 13, 10, 0

input_buffer:
    times 16 db 0

unknown_message:
    db "unknown command!", 13, 10, 0

unknown_command:
    mov si, unknown_message
    call print
    jmp command_loop

help_message:
    db "help: ", 13, 10, 0

help_command:
    mov si, help_message
    call print
    jmp command_loop

halt:
    hlt
    jmp halt

times 1024 - ($ - $$) db 0