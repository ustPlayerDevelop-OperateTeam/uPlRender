// debug_renderer.cpp
// 用来单步调试 Rust 渲染器 DLL 的 C++ 工具程序。
// 纯 Win32 API，不需要 C++ STL（方便 cl.exe 直接编译）。
//
// 用法: debug_renderer.exe [config.json] [fps] [output_dir]
//   config.json  可选，默认 ..\examples\frame_config.json
//   fps          可选，默认 60
//   output_dir   可选，默认 ..\debug_output
//
// 说明:
//   - 从常见位置自动搜索 ustplayer_renderer.dll(当前目录 / target\release / ..\target\release)
//   - 加载全部导出函数(含 up_set_ust_text / up_set_lrc_text)
//   - 先以 1 字节缓冲探测分辨率,再分配正确大小的缓冲

#define UNICODE
#define _UNICODE
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <conio.h>  // _getwch(退出前暂停)

// ---- 从 DLL 导入的函数签名 ----
typedef unsigned __int64 (__stdcall *PFN_up_create_context)(void);
typedef void            (__stdcall *PFN_up_destroy_context)(unsigned __int64);
typedef int             (__stdcall *PFN_up_set_config)(unsigned __int64, const char*);
typedef int             (__stdcall *PFN_up_set_ust_text)(unsigned __int64, const char*);
typedef int             (__stdcall *PFN_up_set_lrc_text)(unsigned __int64, const char*);
typedef int             (__stdcall *PFN_up_begin_export)(unsigned __int64);
typedef int             (__stdcall *PFN_up_render_frame)(unsigned __int64, double);
typedef int             (__stdcall *PFN_up_end_export)(unsigned __int64);
typedef int             (__stdcall *PFN_up_render_to_buffer)(unsigned __int64, double, unsigned char*, int, int*, int*);
typedef void            (__stdcall *PFN_up_set_progress_callback)(unsigned __int64, void (__stdcall *)(int));
typedef const char*     (__stdcall *PFN_up_last_error)(unsigned __int64);

// ---- 全局函数指针 ----
static PFN_up_create_context        g_up_create_context        = nullptr;
static PFN_up_destroy_context       g_up_destroy_context       = nullptr;
static PFN_up_set_config            g_up_set_config            = nullptr;
static PFN_up_set_ust_text          g_up_set_ust_text          = nullptr;
static PFN_up_set_lrc_text          g_up_set_lrc_text          = nullptr;
static PFN_up_begin_export          g_up_begin_export          = nullptr;
static PFN_up_render_frame          g_up_render_frame          = nullptr;
static PFN_up_end_export            g_up_end_export            = nullptr;
static PFN_up_render_to_buffer      g_up_render_to_buffer      = nullptr;
static PFN_up_set_progress_callback g_up_set_progress_callback = nullptr;
static PFN_up_last_error            g_up_last_error            = nullptr;

// ---- 进度回调 ----
static void __stdcall ProgressCallback(int progress) {
    wprintf(L"  progress: %d/1000\n", progress);
}

// ---- BMP24 保存（ bottom-up ） ----
static bool SaveBMP24(const wchar_t* path, const unsigned char* rgba, int w, int h) {
    if (w <= 0 || h <= 0 || rgba == nullptr) return false;

    int rowSize = ((w * 3 + 3) / 4) * 4;
    int imageSize = rowSize * h;
    int fileSize = 14 + 40 + imageSize;

    unsigned char* buf = (unsigned char*)malloc(fileSize);
    if (!buf) return false;
    memset(buf, 0, fileSize);

    // BITMAPFILEHEADER (14 bytes)
    buf[0] = 'B'; buf[1] = 'M';
    memcpy(buf + 2, &fileSize, 4);
    *(unsigned int*)(buf + 10) = 54;

    // BITMAPINFOHEADER (40 bytes)
    *(unsigned int*)(buf + 14) = 40;
    *(int*)(buf + 18) = w;
    *(int*)(buf + 22) = h;
    *(unsigned short*)(buf + 26) = 1;
    *(unsigned short*)(buf + 28) = 24;

    *(unsigned int*)(buf + 34) = (unsigned int)imageSize;

    // pixel data: BGR, bottom-up
    for (int y = h - 1; y >= 0; --y) {
        const unsigned char* src = rgba + (y * w) * 4;
        unsigned char* dst = buf + 54 + (h - 1 - y) * rowSize;
        for (int x = 0; x < w; ++x) {
            dst[x * 3 + 0] = src[x * 4 + 2]; // B
            dst[x * 3 + 1] = src[x * 4 + 1]; // G
            dst[x * 3 + 2] = src[x * 4 + 0]; // R
        }
    }

    HANDLE hFile = CreateFileW(path, GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE) {
        free(buf);
        return false;
    }

    DWORD written = 0;
    BOOL ok = WriteFile(hFile, buf, fileSize, &written, nullptr);
    CloseHandle(hFile);
    free(buf);
    return ok && written == (DWORD)fileSize;
}

// ---- 帮助：读取整个文本文件(宽字符路径) ----
static char* ReadTextFileWide(const wchar_t* path, int* outLen) {
    HANDLE h = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, nullptr,
                           OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return nullptr;
    LARGE_INTEGER size;
    if (!GetFileSizeEx(h, &size) || size.QuadPart <= 0 || size.QuadPart > 64 * 1024 * 1024) {
        CloseHandle(h);
        return nullptr;
    }
    int len = (int)size.QuadPart;
    char* data = (char*)malloc((size_t)len + 1);
    if (!data) { CloseHandle(h); return nullptr; }
    DWORD readed = 0;
    BOOL ok = ReadFile(h, data, (DWORD)len, &readed, nullptr);
    CloseHandle(h);
    if (!ok || (int)readed != len) { free(data); return nullptr; }
    data[len] = '\0';
    if (outLen) *outLen = len;
    return data;
}

// ---- 帮助：从 DLL 取函数指针 ----
template<typename F>
static bool GetProc(HMODULE h, const char* name, F& out) {
    out = (F)GetProcAddress(h, name);
    return out != nullptr;
}

static bool LoadAllFunctions(HMODULE h) {
    if (!GetProc(h, "up_create_context",        g_up_create_context))        return false;
    if (!GetProc(h, "up_destroy_context",       g_up_destroy_context))       return false;
    if (!GetProc(h, "up_set_config",            g_up_set_config))            return false;
    if (!GetProc(h, "up_set_ust_text",          g_up_set_ust_text))          return false;
    if (!GetProc(h, "up_set_lrc_text",          g_up_set_lrc_text))          return false;
    if (!GetProc(h, "up_begin_export",          g_up_begin_export))          return false;
    if (!GetProc(h, "up_render_frame",          g_up_render_frame))          return false;
    if (!GetProc(h, "up_end_export",            g_up_end_export))            return false;
    if (!GetProc(h, "up_render_to_buffer",      g_up_render_to_buffer))      return false;
    if (!GetProc(h, "up_set_progress_callback", g_up_set_progress_callback)) return false;
    if (!GetProc(h, "up_last_error",            g_up_last_error))            return false;
    return true;
}

static void PrintLastError(unsigned __int64 ctx) {
    const char* err = g_up_last_error(ctx);
    if (err && *err) {
        fprintf(stderr, "last_error: %s\n", err);
    } else {
        fprintf(stderr, "last_error: <null>\n");
    }
}

// ---- 搜索 DLL:依次尝试若干候选路径 ----
static HMODULE LoadRendererDll(wchar_t* foundPath, size_t foundPathLen) {
    // 先尝试 exe 所在目录(DLL 与 exe 同放 target\release 时最可靠)
    wchar_t exeDir[MAX_PATH] = {0};
    GetModuleFileNameW(nullptr, exeDir, MAX_PATH);
    wchar_t* slash = wcsrchr(exeDir, L'\\');
    if (slash) *slash = L'\0';

    wchar_t exeDirDll[MAX_PATH];
    swprintf_s(exeDirDll, L"%s\\ustplayer_renderer.dll", exeDir);

    static const wchar_t* candidates[] = {
        L"ustplayer_renderer.dll",
        L"target\\release\\ustplayer_renderer.dll",
        L"..\\target\\release\\ustplayer_renderer.dll",
        L"release\\ustplayer_renderer.dll",
        L"..\\release\\ustplayer_renderer.dll",
    };
    // 1) exe 同目录
    HMODULE h = LoadLibraryW(exeDirDll);
    if (h) {
        if (foundPath) wcscpy_s(foundPath, foundPathLen, exeDirDll);
        return h;
    }
    // 2) 其余候选(相对 cwd)
    for (size_t i = 0; i < sizeof(candidates) / sizeof(candidates[0]); ++i) {
        h = LoadLibraryW(candidates[i]);
        if (h) {
            if (foundPath) wcscpy_s(foundPath, foundPathLen, candidates[i]);
            return h;
        }
    }
    return nullptr;
}

// ---- 退出前暂停:仅当"双击启动"(独占控制台窗口)时,避免窗口闪退 ----
// 用 GetConsoleProcessList 判断:双击启动时控制台里只有本进程(进程数==1);
// 在已有终端(cmd/PowerShell)里运行时进程数>=2(有 shell),此时不暂停。
// 暂停带 5 秒超时:用户不按键也会自动关闭,不会看起来"卡住"。
static void PauseIfDoubleClicked() {
    HANDLE hOut = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD mode = 0;
    if (hOut == INVALID_HANDLE_VALUE || hOut == nullptr || !GetConsoleMode(hOut, &mode)) {
        return; // 输出被重定向(管道/文件),不暂停
    }
    DWORD procList[8] = {0};
    DWORD count = GetConsoleProcessList(procList, 8);
    if (count > 1) {
        return; // 在已有终端里运行,不暂停
    }
    printf("\n========================================\n");
    printf("  Done! 按任意键退出,或 5 秒后自动关闭...\n");
    printf("========================================\n");
    fflush(stdout);
    for (int i = 0; i < 50; ++i) { // 50 * 100ms = 5s
        if (_kbhit()) {
            _getwch();
            return;
        }
        Sleep(100);
    }
}

// ---- 基于 exe 位置推导仓库根目录(target\release\..\..) ----
static void GetRepoRoot(wchar_t* out, size_t outLen) {
    wchar_t exePath[MAX_PATH] = {0};
    GetModuleFileNameW(nullptr, exePath, MAX_PATH);
    wchar_t* slash = wcsrchr(exePath, L'\\');
    if (slash) *slash = L'\0'; // 去掉 exe 文件名 → target\release
    swprintf_s(out, outLen, L"%s\\..\\..", exePath);
}

// ---- main ----
int wmain(int argc, wchar_t* argv[]) {
    if (argc > 4) {
        wprintf(L"用法: debug_renderer.exe [config.json] [fps] [output_dir]\n");
        wprintf(L"  config.json  可选，默认 <仓库根>\\examples\\frame_config.json\n");
        wprintf(L"  fps          可选，默认 60\n");
        wprintf(L"  output_dir   可选，默认 <仓库根>\\debug_output\n");
        PauseIfDoubleClicked();
        return 1;
    }

    // 基于 exe 位置推导仓库根,构造绝对路径默认值(双击运行时 cwd 不可控,不能依赖相对路径)
    wchar_t repoRoot[MAX_PATH] = {0};
    GetRepoRoot(repoRoot, MAX_PATH);

    wchar_t defaultJson[MAX_PATH], defaultOut[MAX_PATH];
    swprintf_s(defaultJson, L"%s\\examples\\frame_config.json", repoRoot);
    swprintf_s(defaultOut, L"%s\\debug_output", repoRoot);

    const wchar_t* jsonPath = (argc >= 2) ? argv[1] : defaultJson;
    int fps = (argc >= 3) ? _wtoi(argv[2]) : 60;
    const wchar_t* outDir = (argc >= 4) ? argv[3] : defaultOut;

    wprintf(L"config  = %s\n", jsonPath);
    wprintf(L"fps     = %d\n", fps);
    wprintf(L"output  = %s\n", outDir);

    // 1. 加载 DLL
    wchar_t dllPath[MAX_PATH] = {0};
    HMODULE hDll = LoadRendererDll(dllPath, MAX_PATH);
    if (!hDll) {
        fprintf(stderr, "无法加载 ustplayer_renderer.dll(已尝试 exe 同目录 / 当前目录 / target\\release / ..\\target\\release)。\n");
        fprintf(stderr, "请先执行: cargo build --release\n");
        PauseIfDoubleClicked();
        return 1;
    }
    wprintf(L"dll     = %s\n", dllPath);

    if (!LoadAllFunctions(hDll)) {
        fprintf(stderr, "DLL 导出函数不完整。\n");
        FreeLibrary(hDll);
        PauseIfDoubleClicked();
        return 1;
    }

    // 2. 读 JSON 配置
    int jsonLen = 0;
    char* json = ReadTextFileWide(jsonPath, &jsonLen);
    if (!json) {
        fprintf(stderr, "读取配置失败: %ls\n", jsonPath);
        FreeLibrary(hDll);
        PauseIfDoubleClicked();
        return 1;
    }

    // 3. 创建上下文
    unsigned __int64 ctx = g_up_create_context();
    if (ctx == 0) {
        fprintf(stderr, "up_create_context 失败\n");
        free(json);
        FreeLibrary(hDll);
        PauseIfDoubleClicked();
        return 1;
    }
    wprintf(L"ctx     = %llu\n", ctx);

    // 4. 设置配置
    int rc = g_up_set_config(ctx, json);
    if (rc != 0) {
        fprintf(stderr, "up_set_config 失败: %d\n", rc);
        PrintLastError(ctx);
        free(json);
        g_up_destroy_context(ctx);
        FreeLibrary(hDll);
        PauseIfDoubleClicked();
        return 1;
    }

    // 注册进度回调(up_render_frame 每次调用会触发)
    if (g_up_set_progress_callback) {
        g_up_set_progress_callback(ctx, ProgressCallback);
    }

    // 4.1 可选:演示 up_set_ust_text / up_set_lrc_text(与 config.ust 相同内容)
    //     这里仅演示调用路径;实际使用时宿主可直接注入已解析的文本。
    if (g_up_set_ust_text && g_up_set_lrc_text) {
        const char* ustDemo =
            "{\"tempo\":120.0,\"notes\":[{\"length\":480,\"lyric\":\"A\",\"note_num\":69}]}";
        const char* lrcDemo = "[00:01.00]第一行\n[00:10.00]第二行";
        int rcUst = g_up_set_ust_text(ctx, ustDemo);
        int rcLrc = g_up_set_lrc_text(ctx, lrcDemo);
        wprintf(L"demo up_set_ust_text=%d up_set_lrc_text=%d\n", rcUst, rcLrc);
    }
    free(json);

    // 5. 开启编码(若 output_path 是 .mp4 则启动 ffmpeg 编码器)
    rc = g_up_begin_export(ctx);
    if (rc != 0) {
        fprintf(stderr, "up_begin_export 失败: %d\n", rc);
        PrintLastError(ctx);
        g_up_destroy_context(ctx);
        FreeLibrary(hDll);
        PauseIfDoubleClicked();
        return 1;
    }

    // 6. 获取分辨率:先用 1 字节缓冲探测(库会先写 out_w/out_h 再报缓冲过小)
    int w = 0, h = 0;
    unsigned char probe = 0;
    int rcProbe = g_up_render_to_buffer(ctx, 0.0, &probe, 1, &w, &h);
    if (w <= 0 || h <= 0) {
        fprintf(stderr, "无法获取分辨率(w=%d h=%d, rc=%d)\n", w, h, rcProbe);
        PrintLastError(ctx);
        g_up_end_export(ctx);
        g_up_destroy_context(ctx);
        FreeLibrary(hDll);
        PauseIfDoubleClicked();
        return 1;
    }
    wprintf(L"分辨率: %dx%d\n", w, h);

    // 判断模式:output_path 以 .mp4 结尾 → 视频编码模式;否则 → BMP 预览模式
    bool videoMode = false;
    {
        // 从原始 JSON 里粗查 output_path 是否以 .mp4 结尾
        int jsonLen2 = 0;
        char* json2 = ReadTextFileWide(jsonPath, &jsonLen2);
        if (json2) {
            const char* key = strstr(json2, "output_path");
            if (key) {
                key += strlen("output_path");
                while (*key && (*key == ' ' || *key == '"' || *key == ':' || *key == '\t' || *key == ',')) key++;
                // 匹配 ".mp4" 或 "out.mp4"
                if (_strnicmp(key, ".mp4", 4) == 0 || _strnicmp(key, "out.mp4", 7) == 0) {
                    videoMode = true;
                }
            }
            free(json2);
        }
    }
    if (videoMode) {
        printf("视频导出模式: 渲染帧并编码到 output_path(ffmpeg)\n");
    } else {
        printf("BMP 预览模式: 渲染帧保存为 BMP(不编码视频)\n");
    }
    fflush(stdout);

    // 分配正确大小的缓冲(仅预览模式需要;视频模式由库内部渲染)
    unsigned char* buf = nullptr;
    size_t bufSize = 0;
    if (!videoMode) {
        bufSize = (size_t)w * (size_t)h * 4;
        if (bufSize > (size_t)INT_MAX) {
            fprintf(stderr, "分辨率过大,无法用 int 长度传入: %dx%d\n", w, h);
            g_up_end_export(ctx);
            g_up_destroy_context(ctx);
            FreeLibrary(hDll);
            PauseIfDoubleClicked();
            return 1;
        }
        buf = (unsigned char*)malloc(bufSize);
        if (!buf) {
            fprintf(stderr, "malloc %zu 字节失败\n", bufSize);
            g_up_end_export(ctx);
            g_up_destroy_context(ctx);
            FreeLibrary(hDll);
            PauseIfDoubleClicked();
            return 1;
        }
    }

    // 确保输出目录存在
    CreateDirectoryW(outDir, nullptr);

    // 7. 渲染 N 帧并保存(单行动态进度条)
    const int totalFrames = fps; // 默认 1 秒，方便快速看
    int saved = 0;
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    for (int i = 0; i < totalFrames; ++i) {
        double t = (double)i / (double)fps;

        int rc2;
        if (videoMode) {
            // 视频模式:up_render_frame 渲染并送入 ffmpeg 编码器
            rc2 = g_up_render_frame(ctx, t);
        } else {
            // 预览模式:up_render_to_buffer 渲染到缓冲,保存 BMP
            rc2 = g_up_render_to_buffer(ctx, t, buf, (int)bufSize, &w, &h);
        }
        if (rc2 != 0) {
            fprintf(stderr, "\nframe %d 渲染失败: %d\n", i, rc2);
            PrintLastError(ctx);
            break;
        }

        wchar_t name[MAX_PATH];
        if (videoMode) {
            swprintf_s(name, L"%s\\video_%d.mp4", outDir, 0); // 仅用于进度条显示
        } else {
            swprintf_s(name, L"%s\\frame_%03d.bmp", outDir, i);
            if (!SaveBMP24(name, buf, w, h)) {
                fprintf(stderr, "\nframe %d 保存 BMP 失败: %ls\n", i, name);
                break;
            }
        }
        saved++;

        // 进度条:同一行刷新(用窄字符 printf,避免宽字符在重定向/locale 下的编码问题)
        int pct = (int)((double)(i + 1) / (double)totalFrames * 100.0);
        int barLen = 24;
        int filled = (int)((double)(i + 1) / (double)totalFrames * barLen);
        char bar[25];
        for (int b = 0; b < barLen; ++b) {
            bar[b] = (b < filled) ? '#' : '-';
        }
        bar[barLen] = '\0';
        if (videoMode) {
            printf("\r[%s] %3d%% (%d/%d) encoding...", bar, pct, i + 1, totalFrames);
        } else {
            printf("\r[%s] %3d%% (%d/%d) frame_%03d.bmp", bar, pct, i + 1, totalFrames, i);
        }
        fflush(stdout);
    }

    QueryPerformanceCounter(&t1);
    double elapsed = (double)(t1.QuadPart - t0.QuadPart) / (double)freq.QuadPart;

    // 8. 结束导出:视频模式下 up_end_export 等待 ffmpeg 完成编码。
    //    提示用户这是正常等待;ffmpeg 的实时进度(frame=/fps=/time=)会输出到终端。
    if (videoMode) {
        printf("\n正在完成视频编码(ffmpeg flush)...\n");
        fflush(stdout);
    }
    g_up_end_export(ctx);
    g_up_destroy_context(ctx);
    free(buf);
    FreeLibrary(hDll);

    printf("\n========================================\n");
    printf("  Done: %d frames", saved);
    if (videoMode) {
        printf(" encoded to MP4");
    } else {
        printf(" saved to %ls", outDir);
    }
    if (elapsed > 0.0 && saved > 0) {
        printf("\n  time %.2fs | avg %.1f ms/frame", elapsed, elapsed * 1000.0 / saved);
    }
    printf("\n========================================\n");
    PauseIfDoubleClicked();
    return 0;
}
