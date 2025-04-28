CC = gcc-11
LD = $(CC) $(KBUILD_CFLAGS) -pthread -lpthread

NOSTDINC_FLAGS :=
LINUXINCLUDE := -I$(src)/include
KBUILD_LDFLAGS :=
KBUILD_CFLAGS := -g -O3 -Wall -D__LINUX_COMPILER_TYPES_H -muintr -mgeneral-regs-only -minline-all-stringops -static

always-y := hello
subdir-y := libs/

ULIBS := $(obj)/libs/ulib.a

$(obj)/%: $(obj)/%.o $(ULIBS) FORCE
	$(call if_changed,ld)
