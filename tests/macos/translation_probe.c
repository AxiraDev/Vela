/* Original Vela test helper, MIT. Reads the OS translation flag of a test process. */
#include <sys/types.h>
#include <sys/sysctl.h>
#include <sys/proc.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    int pid = atoi(argv[1]);
    if (pid <= 0) return 2;
    int mib[] = { CTL_KERN, KERN_PROC, KERN_PROC_PID, pid };
    struct kinfo_proc info;
    size_t size = sizeof(info);
    if (sysctl(mib, 4, &info, &size, NULL, 0) != 0 || size == 0) {
        perror("sysctl KERN_PROC_PID");
        return 1;
    }
    printf("{\"pid\":%d,\"translated\":%s}\n", pid,
           (info.kp_proc.p_flag & P_TRANSLATED) ? "true" : "false");
    return 0;
}
