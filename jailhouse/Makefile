PORT ?= 2334

# Check make version
need := 3.82
ifneq ($(need),$(firstword $(sort $(MAKE_VERSION) $(need))))
$(error Too old make version $(MAKE_VERSION), at least $(need) required)
endif

ifeq ($(V),1)
	Q =
else
	Q = @
endif

# no recipes above this one (also no includes)
all: modules

# out-of-tree build for our kernel-module, firmware and inmates
KDIR ?= /lib/modules/`uname -r`/build

kbuild = -C $(KDIR) M=$$PWD $@

modules clean:
	$(Q)$(MAKE) $(kbuild)

ssh:
	ssh -p $(PORT) ubuntu@localhost

scp_jailhouse:
	scp -P $(PORT) -r ../jailhouse-equation ubuntu@localhost:~/

.PHONY: modules clean ssh
