/* Original Vela integration fixture, MIT. No CRT or third-party Windows SDK. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef int BOOL;
#define WINAPI __stdcall
__declspec(dllimport) HANDLE WINAPI GetStdHandle(DWORD);
__declspec(dllimport) BOOL WINAPI WriteFile(HANDLE, const void *, DWORD, DWORD *, void *);
__declspec(dllimport) void WINAPI Sleep(DWORD);
__declspec(dllimport) __declspec(noreturn) void WINAPI ExitProcess(DWORD);

void mainCRTStartup(void) {
    volatile unsigned int result = 0;
    for (unsigned int i = 0; i < 1000; ++i) result += i;
    if (result != 499500) ExitProcess(3);
#ifdef _WIN64
    static const char message[] = "VELA_WINDOWS_X64_OK\r\n";
#else
    static const char message[] = "VELA_WINDOWS_X86_OK\r\n";
#endif
    DWORD written = 0;
    if (!WriteFile(GetStdHandle((DWORD)-11), message, sizeof(message)-1, &written, 0)) ExitProcess(2);
    /* Keep the real Windows process alive briefly for host-architecture inspection. */
    Sleep(8000);
    ExitProcess(0);
}
