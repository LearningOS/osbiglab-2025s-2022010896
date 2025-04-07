# Arguments
ARCH ?= x86_64
MODE ?= release
LOG ?= warn
RVM ?= on
ACCEL ?= on
UINTR ?= on

# Platform
ifeq ($(ARCH), x86_64)
  ifeq ($(RVM), on)
    PLATFORM ?= pc-rvm
  else
    PLATFORM ?= pc
  endif
else ifeq ($(ARCH), aarch64)
  PLATFORM ?= qemu-virt-arm
endif

export ARCH
export PLATFORM
export MODE
export LOG

make_args := ARCH=$(ARCH) PLATFORM=$(PLATFORM) MODE=$(MODE) LOG=$(LOG)

# Paths
target := ../targets/$(ARCH).json
kernel_elf := target/$(ARCH)/$(MODE)/nimbos
kernel_bin := $(kernel_elf).bin

# Cargo features and build args
features := platform-$(PLATFORM)

ifeq ($(RVM), on)
  features += rvm
endif

build_args := --no-default-features --features "$(features)" --target $(target) -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
ifeq ($(MODE), release)
  build_args += --release
endif

# Binutils
OBJDUMP := rust-objdump -d --print-imm-hex --x86-asm-syntax=intel
OBJCOPY := rust-objcopy --binary-architecture=$(ARCH)
GDB := gdb-multiarch

# QEMU
ifeq ($(UINTR), on)
  qemu := /home/w-43/os/biglab/qemu-uintr/build/x86_64-softmmu/qemu-system-x86_64
else
  qemu := qemu-system-$(ARCH)
endif
qemu_args := -nographic -m 128M


ifeq ($(ARCH), x86_64)
  ifeq ($(UINTR), on)
  	qemu_args += -machine q35,kernel_irqchip=split \
		-cpu qemu64 \
		-serial mon:stdio \
		-kernel $(kernel_elf)
  else
	ifeq ($(ACCEL), on)
		qemu_args += -cpu host -accel kvm
	endif
	qemu_args += \
		-machine q35 \
		-serial mon:stdio \
		-kernel $(kernel_elf)
  endif
else ifeq ($(ARCH), aarch64)
  qemu_args += \
    -cpu cortex-a72 \
    -machine virt \
    -kernel $(kernel_bin)
endif

# GDB
GDB := gdb-multiarch

build: $(kernel_bin)

env:
	cargo install cargo-binutils --vers =0.3.3
	rustup component add rust-src
	rustup component add llvm-tools-preview

$(kernel_bin): kernel
	@$(OBJCOPY) $(kernel_elf) --strip-all -O binary $@

user:
	@cd ../user && make build

kernel:
	@echo Arch: $(ARCH), Platform: $(PLATFORM)
ifneq ($(shell cat .makeargs), $(make_args))
	@echo $(make_args) > .makeargs
endif
	cargo build $(build_args)

clean:
	@cd ../user && make clean
	@cargo clean

clippy:
	cargo clippy $(build_args)

disasm:
	@$(OBJDUMP) $(kernel_elf) | less

run: user build justrun

justrun:
	$(qemu) $(qemu_args)

debug: build
	$(qemu) $(qemu_args) -s -S &
	sleep 1
	$(GDB) $(kernel_elf) -ex 'target remote localhost:1234'

scp:
	scp -P 2333 $(kernel_bin) ubuntu@localhost:/home/ubuntu

.PHONY: build env kernel user clean disasm run debug scp
