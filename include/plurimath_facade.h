#ifndef PLURIMATH_FACADE_H
#define PLURIMATH_FACADE_H
#include <stddef.h>
#include <stdint.h>
uint32_t plurimath_facade_abi_version(void);
/* UTF-8 JSON request, length-delimited. Caller retains input. Result is an owned UTF-8 NUL-terminated JSON string; free once. */
char *plurimath_facade_json(const uint8_t *input, size_t length);
void plurimath_facade_free(char *result);
#endif
