#include <stdio.h>
#include <unistd.h>
#include <syslog.h>

#include <nimbos.h>

int main()
{
    openlog("my_program", LOG_PID | LOG_CONS, LOG_USER);
    printf("Hello NimbOS!\n");

    int fd = nimbos_setup_syscall();
    if (fd <= 0) {
        printf("Failed to open NimbOS device `%s`\n", NIMBOS_DEV);
        return fd;
    }

    for (;;) {
        // printf("Sleep %d...\n", i);
        usleep(1000);
    }

    close(fd);
    closelog();
    return 0;
}
