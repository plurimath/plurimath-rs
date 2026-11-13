#include "plurimath_numeric.h"
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
  if (argc != 2 || plurimath_numeric_abi_version() != 1) return 2;
  char *wire = plurimath_numeric_evaluate((const uint8_t *)argv[1], strlen(argv[1]));
  if (!wire) return 3;
  puts(wire);
  plurimath_numeric_result_free(wire);
  return 0;
}
