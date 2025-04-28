# QEMU ?= qemu-system-x86_64
QEMU ?= /home/w-43/os/biglab/qemu-uintr/build/x86_64-softmmu/qemu-system-x86_64
PORT ?= 2333
UINTR ?= off

qemu_image := ubuntu-20.04-server-cloudimg-amd64.img
ifeq ($(UINTR), off)
  qemu_args := \
	-smp 4 -m 8G -accel kvm -nographic \
	-machine q35,kernel_irqchip=split \
	-cpu host,-waitpkg,-kvm-asyncpf,-kvm-pv-eoi,-kvm-pv-ipi,-kvm-pv-sched-yield,-kvm-pv-unhalt,-kvm-steal-time,-kvmclock \
	-drive file=$(qemu_image) \
	-net user,id=net,hostfwd=tcp::$(PORT)-:22 -net nic,model=e1000e \
	-serial mon:stdio
else
  qemu_args := -smp 4  \
	-machine q35,kernel_irqchip=split \
	-m 8G   -nographic \
	-cpu qemu64 \
	-drive file=$(qemu_image) \
	-net user,id=net,hostfwd=tcp::$(PORT)-:22 -net nic,model=e1000e \
	-serial mon:stdio
endif
# ubuntu := "/home/w-43/os/biglab/uintr-linux-kernel/ubuntu-x86_64.cpio.gz"
# kernel := "/home/w-43/uintr-linux-kernel/build/arch/x86_64/boot/bzImage"


# -kernel "/home/w-43/os/biglab/uintr-linux-kernel/build/arch/x86_64/boot/bzImage" \
# -initrd "/home/w-43/os/biglab/uintr-linux-kernel/ubuntu-x86_64.cpio.gz" \
# -append "root=/dev/ram0 rw rootfstype=ext4 console=ttyS0 init=/linuxrc" \

$(qemu_image):
	wget https://cloud-images.ubuntu.com/releases/focal/release/$(qemu_image)

.ONESHELL:
image: $(qemu_image)
	cat >user-data <<EOF
	#cloud-config
	password: guest
	chpasswd: { expire: False }
	ssh_pwauth: True
	EOF
	cloud-localds user-data.img user-data
	qemu-img resize $(qemu_image) +64G
	$(QEMU) $(qemu_args) -drive file=user-data.img,format=raw

qemu: $(qemu_image)
	$(QEMU) $(qemu_args)

ssh:
	ssh -p $(PORT) ubuntu@localhost
