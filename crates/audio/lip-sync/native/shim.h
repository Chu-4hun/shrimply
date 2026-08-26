#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    int64_t start_centiseconds;
    int64_t end_centiseconds;
    uint8_t shape;
} ShrimplyMouthCue;

typedef struct {
    ShrimplyMouthCue* cues;
    size_t cue_count;
    char* error;
} ShrimplyRhubarbResult;

int shrimply_rhubarb_analyze(
    const char* wave_path,
    const char* model_directory,
    int max_thread_count,
    ShrimplyRhubarbResult* result
);

void shrimply_rhubarb_free_result(ShrimplyRhubarbResult* result);

#ifdef __cplusplus
}
#endif

