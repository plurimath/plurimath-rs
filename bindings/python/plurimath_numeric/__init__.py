"""Thin ABI-v1 consumer. The Rust library owns all numeric semantics."""
import ctypes
import json

class NumericKernel:
    def __init__(self, library_path):
        self._library = ctypes.CDLL(str(library_path))
        self._library.plurimath_numeric_abi_version.restype = ctypes.c_uint32
        if self._library.plurimath_numeric_abi_version() != 1:
            raise RuntimeError("unsupported numeric kernel ABI")
        self._library.plurimath_numeric_evaluate.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        self._library.plurimath_numeric_evaluate.restype = ctypes.c_void_p
        self._library.plurimath_numeric_result_free.argtypes = [ctypes.c_void_p]
        self._library.plurimath_numeric_result_free.restype = None

    def evaluate_json(self, text):
        data = text.encode("utf-8")
        source = ctypes.create_string_buffer(data)
        pointer = self._library.plurimath_numeric_evaluate(source, len(data))
        if not pointer:
            raise RuntimeError("numeric kernel runtime failure")
        try:
            return ctypes.string_at(pointer).decode("utf-8")
        finally:
            self._library.plurimath_numeric_result_free(pointer)

    def evaluate(self, request):
        return json.loads(self.evaluate_json(json.dumps(request, ensure_ascii=False, separators=(",", ":"))))
