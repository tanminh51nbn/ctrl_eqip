#ifndef CTRL_EQIP_H
#define CTRL_EQIP_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- Constants ---

/** Distance bounds (meters) defining each category. */
#define CE_CLOSE_THRESHOLD_M 1.0
#define CE_MEDIUM_THRESHOLD_M 3.0
#define CE_FAR_THRESHOLD_M 5.0

/** Default average standing person height (meters). */
#define CE_DEFAULT_PERSON_HEIGHT_M 1.70

/** Frame delimiter constants matching PROTOCOL_SPEC.md */
#define CE_FRAME_START 0xAA
#define CE_FRAME_END   0x55

/** Baud rate used on both the Laptop and ESP32-C3 sides. */
#define CE_BAUD_RATE 115200

// --- Types ---

/** Opaque handle representing the AI Detection Pipeline. */
typedef void* CePipelineHandle;

/** C-compatible tracking result for a single frame. */
typedef struct {
    uint32_t person_count;       /** Total number of detected people. */
    float closest_distance_m;    /** Meters to the closest person (0.0 if not found). */
    bool has_person;             /** Whether any person is present. */
} CeTrackingResult;

// --- API Functions ---

/** 
 * Starts the AI detection pipeline.
 * @param model_path Null-terminated string to the ONNX model file.
 * @param width Camera width (e.g. 640).
 * @param height Camera height (e.g. 480).
 * @return A handle to the pipeline, or NULL on failure.
 */
CePipelineHandle ce_pipeline_start(const char* model_path, uint32_t width, uint32_t height);

/** 
 * Polls the latest detection result.
 * @return 1 if a result was received, 0 if no new data.
 */
int ce_pipeline_try_recv(CePipelineHandle handle, CeTrackingResult* out_result);

/** 
 * Stops the pipeline and releases all resources/threads.
 */
void ce_pipeline_stop(CePipelineHandle handle);

/** 
 * Returns the library version string. Memory must be freed with ce_free_string.
 */
char* ce_get_version();

/** 
 * Frees a string allocated by the library.
 */
void ce_free_string(char* s);

#ifdef __cplusplus
}
#endif

#endif /* CTRL_EQIP_H */
