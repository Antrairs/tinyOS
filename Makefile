ASM := nasm
QEMU := qemu-system-i386
PYTHON := python3

BOOT_SRC := boot.asm
KERNEL_SRC := kernel.asm

BUILD_DIR := build
BOOT_BIN := $(BUILD_DIR)/boot.bin
KERNEL_BIN := $(BUILD_DIR)/kernel.bin
IMAGE := $(BUILD_DIR)/floppy.img
DATA_IMAGE := $(BUILD_DIR)/data.img

.PHONY: build run clean

run: build
	$(QEMU) -drive file=build/floppy.img,format=raw,if=floppy,index=0 -drive file=build/data.img,format=raw,if=floppy,index=1

build: $(IMAGE)
	@if [ ! -f "$(DATA_IMAGE)" ]; then \
		$(PYTHON) tools/mkfat12.py "$(DATA_IMAGE)"; \
	else \
		echo "Using existing $(DATA_IMAGE)"; \
	fi

$(BOOT_BIN): $(BOOT_SRC)
	mkdir -p $(BUILD_DIR)
	$(ASM) -f bin $(BOOT_SRC) -o $(BOOT_BIN)

$(KERNEL_BIN): $(KERNEL_SRC)
	mkdir -p $(BUILD_DIR)
	$(ASM) -f bin $(KERNEL_SRC) -o $(KERNEL_BIN)

$(IMAGE): $(BOOT_BIN) $(KERNEL_BIN)
	dd if=/dev/zero of=$(IMAGE) bs=512 count=2880 status=none
	dd if=$(BOOT_BIN) of=$(IMAGE) bs=512 seek=0 conv=notrunc status=none
	dd if=$(KERNEL_BIN) of=$(IMAGE) bs=512 seek=1 conv=notrunc status=none

clean:
	rm -f $(BOOT_BIN) $(KERNEL_BIN) $(IMAGE)
