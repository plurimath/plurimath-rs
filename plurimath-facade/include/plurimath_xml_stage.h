#ifndef PLURIMATH_XML_STAGE_H
#define PLURIMATH_XML_STAGE_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Native-only preview, built with the xml-stages Cargo feature.
 * Borrow input for this call. Result is owned UTF-8 JSON; release exactly
 * once with plurimath_facade_free from plurimath_facade.h. Maximum 16 MiB. */
char *plurimath_facade_xml_stage_json(const uint8_t *input, size_t length);
#ifdef __cplusplus
}
#endif
#endif
