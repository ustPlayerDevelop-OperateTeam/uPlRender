/* ustplayer_renderer.h - C ABI 头文件(手写,与 src/ffi.rs 导出保持同步) */
#ifndef USTPLAYER_RENDERER_H
#define USTPLAYER_RENDERER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---------- 错误码(负值一律为错误) ---------- */
enum UstPlayerErr {
    UP_OK = 0,
    UP_ERR_INVALID_ARG = -1, /* 空指针 / 非法枚举 / 长度越界 */
    UP_ERR_PARSE = -2,       /* UST / LRC / JSON 解析失败 */
    UP_ERR_IO = -3,          /* 文件读写失败 */
    UP_ERR_FONT = -4,        /* 字体加载失败 */
    UP_ERR_RENDER = -5,      /* 渲染阶段失败 */
    UP_ERR_ENCODE = -6,      /* 编码器初始化 / 写入失败 */
    UP_ERR_INTERNAL = -99,   /* 未分类内部错误(已捕获的 panic) */
};

/* ---------- 进度回调(千分比 0~1000) ---------- */
typedef void (*UstPlayerProgressFn)(int32_t progress);

/* ---------- 句柄生命周期 ---------- */
/* 创建渲染上下文:一次创建,多次复用(内部缓存字体、LRC 等)。失败返回 0。 */
uint64_t up_create_context(void);

/* 销毁渲染上下文,释放内部所有资源。 */
void up_destroy_context(uint64_t ctx);

/* 设置渲染配置(JSON 文本,UTF-8),返回错误码。 */
int32_t up_set_config(uint64_t ctx, const char* json_config);

/* 直接传入 UST JSON(UTF-8),替换 config.ust 并重建时序模型。 */
int32_t up_set_ust_text(uint64_t ctx, const char* ust_json);

/* 直接传入 LRC 文本(UTF-8/GBK 等,内部多编码探测),替换 lrc_path 读取结果。 */
int32_t up_set_lrc_text(uint64_t ctx, const char* lrc_text);

/* ---------- 视频导出 ---------- */
/* 按当前配置(宽/高/fps/输出路径)初始化编码器。 */
int32_t up_begin_export(uint64_t ctx);

/* 渲染并送入一帧。elapsed_sec 为这一帧的播放时间(秒);-1 表示取默认递增。 */
int32_t up_render_frame(uint64_t ctx, double elapsed_sec);

/* 结束导出:flush 编码器、写尾部、关闭文件。 */
int32_t up_end_export(uint64_t ctx);

/* 设置编码进度回调(千分比 0~1000)。 */
void up_set_progress_callback(uint64_t ctx, UstPlayerProgressFn cb);

/* ---------- 单帧离屏渲染(不编码,供预览) ---------- */
/* 渲染单帧到宿主提供的 RGBA8888(premultiplied)缓冲。
   buf 由宿主分配,长度 >= width*height*4。返回错误码。 */
int32_t up_render_to_buffer(uint64_t ctx, double elapsed_sec,
                            uint8_t* buf, int32_t buf_len,
                            int32_t* out_w, int32_t* out_h);

/* ---------- 错误消息 ---------- */
/* 取最近一次错误消息(UTF-8,静态缓冲)。返回字符串指针,宿主不可 free;
   下次调用任何 up_* 前内容仍有效。无错误时返回 NULL。 */
const char* up_last_error(uint64_t ctx);

/* ---------- 内存与线程约定 ----------
 * - 除 up_render_to_buffer 的 buf 由宿主提供外,其余字符串/句柄一律由库分配、库释放;
 *   宿主不得 free 任何库返回的指针。
 * - 不同 ctx 之间可并发调用(内部各自独立);同一 ctx 的所有调用必须串行。
 * - 所有跨 FFI 字符串统一 UTF-8 编码。
 * - 所有导出函数绝不 panic;非法参数一律返回负错误码。 */

#ifdef __cplusplus
}
#endif

#endif /* USTPLAYER_RENDERER_H */
