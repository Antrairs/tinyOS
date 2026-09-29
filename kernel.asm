bits 16
org 0x8000

KERNEL_SECTORS equ 17
INPUT_BUFFER_SIZE equ 80

start:
    ; 引导程序在实模式下跳到 0000:8000
    cli
    xor ax, ax
    mov ss, ax
    mov sp, 0x7c00
    mov ds, ax
    mov es, ax
    mov [boot_drive], dl
    cld
    sti

    mov si, message
    call print
    
    mov ax, 0
    mov es, ax
    mov bx, 0x9000

    mov ch, 0
    mov dh, 0
    mov cl, 1
    call read_sector
    jc read_failed

    cmp byte [0x91fe], 0x55
    jne bad_signature_error
    cmp byte [0x91ff], 0xaa
    jne bad_signature_error

    mov si, read_ok_message
    call print

command_loop:
    mov si, prompt
    call print
    mov di, input_buffer

input_loop:
    mov ah, 0
    int 0x16
    ; 处理退格键
    cmp al, 8
    je backspace
    ; 处理回车键
    cmp al, 13
    je finish_input
    cmp di, input_buffer + INPUT_BUFFER_SIZE - 1
    jae input_loop    ; DI 已到最后一个字节：不保存，也不显示
    mov [di], al      ; 保存当前字符
    inc di            ; 指向下一个空位置
    mov ah, 0x0e
    int 0x10
    jmp input_loop

backspace:
    ; 已在缓冲区开头, 忽略退格
    cmp di, input_buffer
    je input_loop

    dec di
    mov byte [di], 0

    ; 光标左移, 擦掉字符, 左移回空位
    mov ah, 0x0e
    mov al, 8
    int 0x10
    mov al, ' '
    int 0x10
    mov al, 8
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

    cmp byte [input_buffer], 0
    je command_loop

    mov si, input_buffer
    call split_command
    mov si, command_table

.dispatch_next:   ; 检查命令
    lodsw         ; 从 [ds:si] 读取 16 位数到 ax, 并让 si 前进两个字节
    test ax, ax   ; 检查 ax 是否为 0
    jz unknown_command
    
    mov di, ax    ; di存表中的命令名
    lodsw
    mov bx, ax    ; bx存对应的处理函数

    push si
    mov si, input_buffer
    call strcmp   ; 字符串比较函数
    pop si

    test al, al
    jnz .dispatch_found
    jmp .dispatch_next

.dispatch_found:
    jmp bx

strcmp:
.next:
    mov al, [si]
    cmp al, [di]
    jne .neq
    test al, al
    jz .eq
    inc si
    inc di
    jmp .next
.eq:
    mov al, 1
    ret
.neq:
    xor al, al
    ret

split_command:
    mov word [arg_ptr], 0
.find_space:
    cmp byte [si], 0
    je .done
    cmp byte [si], ' '
    je .found_space
    inc si
    jmp .find_space
.found_space:
    mov byte [si], 0
    inc si
.skip_spaces:
    cmp byte [si], ' '
    jne .save_arument
    inc si
    jmp .skip_spaces
.save_arument:
    cmp byte [si], 0
    je .done
    mov [arg_ptr], si
.done:
    ret


bad_signature_error:
    mov si, bad_signature
    call print
    jmp command_loop

read_sector:
    mov dl, [data_drive]
    mov ah, 0x02
    mov al, 1
    int 0x13
    ret

read_failed:
    mov si, read_error_message
    call print
    jmp command_loop

write_sector:
    mov dl, [data_drive]
    mov ah, 0x03
    mov al, 1
    int 0x13
    ret

write_failed:
    mov si, write_error_message
    call print
    jmp command_loop

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

boot_drive:
    db 0

data_drive:
    db 1

command_table:
    dw sym_help, help_command
    dw 0, 0

arg_ptr:
    dw 0

sym_help:
    db "help", 0

help_command:
    cmp word [arg_ptr], 0
    je .show_help
    jmp unknown_command
.show_help:
    mov si, .help_message
    call print
    jmp command_loop
.help_message:
    db "help: ", 13, 10, 0

read_ok_message:
    db "sector read OK", 13, 10, 0

read_error_message:
    db "sector read FAILED", 13, 10, 0

write_ok_message:
    db "sector write OK", 13, 10, 0

write_error_message:
    db "sector write FAILED", 13, 10, 0

bad_signature:
    db "bad signature!", 13, 10, 0

input_buffer:
    times INPUT_BUFFER_SIZE db 0

unknown_message:
    db "unknown command!", 13, 10, 0

unknown_command:
    mov si, unknown_message
    call print
    jmp command_loop

halt:
    hlt
    jmp halt

times (KERNEL_SECTORS * 512) - ($ - $$) db 0