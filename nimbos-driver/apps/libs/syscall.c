#include <assert.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <sys/ioctl.h>
#include <unistd.h>
#include <syslog.h>

#define _GNU_SOURCE
#include <stdlib.h>
#include <syscall.h>
#include <stdint.h>
#include <x86gprintrin.h>

#include "nimbos.h"
#include "scf.h"

#define __NR_uintr_register_handler	449
#define __NR_uintr_unregister_handler	450
#define __NR_uintr_create_fd		451
#define __NR_uintr_register_sender	452
#define __NR_uintr_unregister_sender	453

/* For simiplicity, until glibc support is added */
#define uintr_register_handler(handler, flags)	syscall(__NR_uintr_register_handler, handler, flags)
#define uintr_unregister_handler(flags)		syscall(__NR_uintr_unregister_handler, flags)
#define uintr_create_fd(vector, flags)		syscall(__NR_uintr_create_fd, vector, flags)
#define uintr_register_sender(fd, flags)	syscall(__NR_uintr_register_sender, fd, flags)
#define uintr_unregister_sender(fd, flags)	syscall(__NR_uintr_unregister_sender, fd, flags)

#define UINTR_GET_UPID_PHYS_ADDR _IOR('u', 1, uint64_t)

void __attribute__ ((interrupt)) uintr_handler(struct __uintr_frame *ui_frame,
    unsigned long long vector)
{
    static const char print[] = "\t-- Linux User Interrupt handler --\n";
    write(STDOUT_FILENO, print, sizeof(print) - 1);
}

struct read_write_args {
    int fd;
    uint64_t buf_offset;
    uint64_t len;
};

static void *read_thread_fn(void *arg)
{
    struct read_write_args *args;
    struct syscall_queue_buffer *scf_buf = get_syscall_queue_buffer();
    uint16_t desc_index = (uint16_t)(long)arg;
    struct scf_descriptor *desc = get_syscall_request_from_index(scf_buf, desc_index);

    if (!desc) {
        return NULL;
    }

    args = offset_to_ptr(desc->args);
    char *buf = offset_to_ptr(args->buf_offset);
    int ret = read(args->fd, buf, args->len);
    // assert(ret == args->len);
    push_syscall_response(scf_buf, desc_index, ret);
    return NULL;
}

static void poll_requests(void)
{
    uint16_t desc_index;
    struct scf_descriptor desc;
    struct syscall_queue_buffer *scf_buf = get_syscall_queue_buffer();
    pthread_t thread; // FIXME: use global threads pool

    while (!pop_syscall_request(scf_buf, &desc_index, &desc)) {
        // printf("syscall: desc_index=%d, opcode=%d, args=0x%lx\n", desc_index,
        // desc.opcode, desc.args);
        switch (desc.opcode) {
        case IPC_OP_READ: {
            // printf("handling IPC_OP_READ");
            syslog(LOG_INFO, "handling IPC_OP_READ");
            pthread_create(&thread, NULL, read_thread_fn, (void *)(long)desc_index);
            break;
        }
        case IPC_OP_WRITE: {
            // printf("handling IPC_OP_WRITE");
            syslog(LOG_INFO, "handling IPC_OP_WRITE");
            struct read_write_args *args = offset_to_ptr(desc.args);
            char *buf = offset_to_ptr(args->buf_offset);
            int ret = write(args->fd, buf, args->len);
            assert(ret == args->len);
            push_syscall_response(scf_buf, desc_index, ret);
            break;
        }
        case ICP_OP_INIT_UINTR: {
            syslog(LOG_INFO, "handling ICP_OP_INIT_UINTR");
            int uipi_index;
            // printf("desc.args %lx\n", desc.args);
            uint64_t nimbos_upid_addr = (desc.args - 0xffffff903a000000UL) + 0xffffff0000000000UL;
            printf("calculated UPID addr %lx\n", nimbos_upid_addr);
	        uipi_index = uintr_register_sender(nimbos_upid_addr, 1<<9);
            if (uipi_index < 0) {
                printf("Sender register error\n");
                push_syscall_response(scf_buf, desc_index, 0);
                break;
            }
            // printf("UITTE index: %d\n", uipi_index);
            _senduipi(uipi_index);

            if (uintr_register_handler(uintr_handler, 0)) {
                printf("Interrupt handler register error\n");
                push_syscall_response(scf_buf, desc_index, 0);
                break;
            }
        
            int uintr_fd = uintr_create_fd(0, 0);
            if (uintr_fd < 0) {
                printf("Interrupt vector allocation error\n");
                push_syscall_response(scf_buf, desc_index, 0);
                break;
            }
            // 2. 获取 UPID 地址
	        uint64_t upid_addr;
            if (ioctl(uintr_fd, UINTR_GET_UPID_PHYS_ADDR, &upid_addr) < 0) {
                printf("ioctl failed\n");
                close(uintr_fd);
                push_syscall_response(scf_buf, desc_index, 0);
                break;
            }
            // 3. 打印 UPID 地址
            printf("Linux UPID address: 0x%llx\n", (unsigned long long)upid_addr);

            _stui();
            push_syscall_response(scf_buf, desc_index, upid_addr);
            break;
        }
        default:
            break;
        }
    }
}

static void nimbos_syscall_handler(int signum)
{
    if (signum == NIMBOS_SYSCALL_SIG_NUM) {
        poll_requests();
    }
}

int nimbos_setup_syscall()
{
    int fd = open(NIMBOS_DEV, O_RDWR);
    if (fd <= 0) {
        return fd;
    }
    int err = nimbos_setup_syscall_buffers(fd);
    if (err) {
        fprintf(stderr, "Failed to setup syscall buffers: %d\n", err);
        return err;
    }

    ioctl(fd, NIMBOS_SETUP_SYSCALL);
    signal(NIMBOS_SYSCALL_SIG_NUM, nimbos_syscall_handler);

    // handle requests before app starting
    poll_requests();

    return fd;
}
