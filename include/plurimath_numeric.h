#ifndef PLURIMATH_NUMERIC_H
#define PLURIMATH_NUMERIC_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* ABI 1. Input UTF-8 is borrowed for the call. A non-NULL result owns one
 * NUL-terminated UTF-8 JSON buffer. Release exactly once with result_free.
 * NULL reports a native runtime failure. Invalid JSON/UTF-8 returns the
 * canonical InvalidRequest envelope. Pointers must address valid memory. */
uint32_t plurimath_numeric_abi_version(void);
char *plurimath_numeric_evaluate(const uint8_t *input, size_t len);
void plurimath_numeric_result_free(char *result);
uint8_t *plurimath_numeric_alloc(size_t len);
void plurimath_numeric_dealloc(uint8_t *ptr, size_t len);
#ifdef __cplusplus
}
#endif
#endif
